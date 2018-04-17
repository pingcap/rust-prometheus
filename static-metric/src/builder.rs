// Copyright 2018 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::atomic::{AtomicUsize, Ordering};

use quote::Tokens;
use syn::{Expr, Ident, LitStr};

use super::parser::*;
use super::util;

lazy_static! {
    /// Used for isolating different static metrics, so that structs for labels in each metric will not conflict even
    /// when they have a common prefix.
    static ref SCOPE_ID: AtomicUsize = AtomicUsize::new(0);
}

pub struct TokensBuilder;

impl TokensBuilder {
    pub fn build(macro_body: &StaticMetricMacroBody) -> Tokens {
        let mut tokens = Tokens::new();
        for m in macro_body.metrics.iter() {
            tokens.append_all(Self::build_static_metric(m));
        }
        tokens
    }

    fn build_static_metric(metric: &MetricDef) -> Tokens {
        let label_struct: Vec<Tokens> = metric
            .labels
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let builder_context = MetricBuilderContext::new(metric, i);
                let code_struct = builder_context.build_struct();
                let code_impl = builder_context.build_impl();
                quote!{
                    #code_struct
                    #code_impl
                }
            })
            .collect();

        let scope_id = SCOPE_ID.fetch_add(1, Ordering::Relaxed);
        let scope_name = Ident::from(format!("prometheus_static_scope_{}", scope_id));

        let visibility = &metric.visibility;
        let struct_name = &metric.struct_name;
        let metric_type = &metric.metric_type;
        let metric_vec_type = util::get_metric_vec_type(metric_type);

        quote!{
            #visibility use self::#scope_name::#struct_name;

            mod #scope_name {
                use std::collections::HashMap;
                use prometheus::#metric_type;
                use prometheus::#metric_vec_type;

                #[allow(unused_imports)]
                use super::*;

                #(
                    #label_struct
                )*
            }
        }
    }
}

struct MetricBuilderContext<'a> {
    metric: &'a MetricDef,
    label: &'a MetricLabelDef,
    label_index: usize,
    is_last_label: bool,

    struct_name: Ident,
    metric_vec_type: Ident,
    member_type: Ident,
}

impl<'a> MetricBuilderContext<'a> {
    fn new(metric: &'a MetricDef, label_index: usize) -> MetricBuilderContext<'a> {
        let is_last_label = label_index == metric.labels.len() - 1;
        MetricBuilderContext {
            metric,
            label: &metric.labels[label_index],
            label_index,
            is_last_label,

            struct_name: util::get_label_struct_name(&metric.struct_name, label_index),
            metric_vec_type: util::get_metric_vec_type(&metric.metric_type),
            member_type: util::get_member_type(
                &metric.struct_name,
                label_index,
                &metric.metric_type,
                is_last_label,
            ),
        }
    }

    fn build_struct(&self) -> Tokens {
        let visibility = &self.metric.visibility;
        let struct_name = &self.struct_name;

        let field_names: Vec<&Ident> = self.label.values.iter().map(|v| &v.name).collect();
        let member_types: Vec<&Ident> = field_names.iter().map(|_| &self.member_type).collect();

        quote!{
            #[allow(missing_copy_implementations)]
            #visibility struct #struct_name {
                #(
                    pub #field_names: #member_types,
                )*
            }
        }
    }

    fn build_impl(&self) -> Tokens {
        let struct_name = &self.struct_name;
        let impl_from = self.build_impl_from();
        let impl_get_by_label = self.build_impl_get();
        quote!{
            impl #struct_name {
                #impl_from
                #impl_get_by_label
            }
        }
    }

    fn build_impl_from(&self) -> Tokens {
        let struct_name = &self.struct_name;
        let metric_vec_type = &self.metric_vec_type;

        let prev_labels_ident: Vec<Ident> = (0..self.label_index)
            .map(|i| Ident::from(format!("label_{}", i)))
            .collect();
        let body = self.build_impl_from_body(prev_labels_ident.clone());

        quote!{
            pub fn from(
                #(
                    #prev_labels_ident: &str,
                )*
                m: &#metric_vec_type
            ) -> #struct_name {
                #struct_name {
                    #body
                }
            }
        }
    }

    fn build_impl_from_body(&self, prev_labels_ident: Vec<Ident>) -> Tokens {
        let member_type = &self.member_type;
        let bodies: Vec<Tokens> = self.label
            .values
            .iter()
            .map(|value| {
                let name = &value.name;
                let value = &value.value;
                if self.is_last_label {
                    let current_label = &self.label.label_key;
                    let prev_labels_str: Vec<&LitStr> = prev_labels_ident
                        .iter()
                        .enumerate()
                        .map(|(i, _)| &self.metric.labels[i].label_key)
                        .collect();
                    let prev_labels_ident = prev_labels_ident.clone();
                    quote!{
                        #name: m.with(&{
                            let mut coll = HashMap::new();
                            #(
                                coll.insert(#prev_labels_str, #prev_labels_ident);
                            )*
                            coll.insert(#current_label, #value);
                            coll
                        }),
                    }
                } else {
                    let prev_labels_ident = prev_labels_ident.clone();
                    quote!{
                        #name: #member_type::from(
                            #(
                                #prev_labels_ident,
                            )*
                            #value,
                            m,
                        ),
                    }
                }
            })
            .collect();
        quote!{
            #(
                #bodies
            )*
        }
    }

    fn build_impl_get(&self) -> Tokens {
        let member_type = &self.member_type;
        let values_str: Vec<&Expr> = self.label.values.iter().map(|v| &v.value).collect();
        let names_ident: Vec<&Ident> = self.label.values.iter().map(|v| &v.name).collect();
        quote!{
            pub fn get(&self, value: &str) -> &#member_type {
                match value {
                    #(
                        #values_str => &self.#names_ident,
                    )*
                    _ => panic!("unknown field `{}`", value),
                }
            }
        }
    }
}
