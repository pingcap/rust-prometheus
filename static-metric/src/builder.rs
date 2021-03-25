// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use proc_macro2::{Span, TokenStream as Tokens};
use quote::TokenStreamExt;
use syn::{Ident, Visibility};

use quote::quote;

use super::parser::*;
use super::util;

/// Used for isolating different static metrics, so that structs for labels in each metric will not conflict even
/// when they have a common prefix.
pub static SCOPE_ID: AtomicUsize = AtomicUsize::new(0);

pub struct TokensBuilder;

impl TokensBuilder {
    pub fn build(macro_body: StaticMetricMacroBody) -> Tokens {
        let mut enums_definitions = HashMap::new();
        let mut tokens = Tokens::new();
        for item in macro_body.items {
            match item {
                StaticMetricMacroBodyItem::Metric(m) => {
                    // If this is a metric definition, expand to a `struct`.
                    tokens.append_all(Self::build_metric_struct(&m, &enums_definitions));
                }
                StaticMetricMacroBodyItem::Enum(e) => {
                    // If this is a label enum definition, expand to an `enum` and
                    // add to the collection.
                    tokens.append_all(Self::build_label_enum(&e));
                    enums_definitions.insert(e.enum_name.clone(), e);
                }
            }
        }
        tokens
    }

    fn build_metric_struct(
        metric: &MetricDef,
        enum_definitions: &HashMap<Ident, MetricEnumDef>,
    ) -> Tokens {
        // Check `label_enum` references.
        for label in &metric.labels {
            let enum_ident = label.get_enum_ident();
            if let Some(e) = enum_ident {
                // If metric is using a `label_enum`, it must exist before the metric definition.
                let enum_def = enum_definitions.get(e);
                if enum_def.is_none() {
                    panic!("Label enum `{}` is undefined.", e)
                }

                // If metric has `pub` visibility, then `label_enum` should also be `pub`.
                // TODO: Support other visibility, like `pub(xx)`.
                if let Visibility::Public(_) = metric.visibility {
                    if let Visibility::Public(_) = enum_def.unwrap().visibility {
                        // `pub` is ok.
                    } else {
                        // others are unexpected.
                        panic!(
                            "Label enum `{}` does not have enough visibility because it is \
                             used in metric `{}` which has `pub` visibility.",
                            e, metric.struct_name
                        );
                    }
                }
            }
        }

        let label_struct: Vec<_> = metric
            .labels
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let builder_context = MetricBuilderContext::new(metric, enum_definitions, i);
                let code_struct = builder_context.build_struct();
                let code_impl = builder_context.build_impl();
                let code_trait_impl = builder_context.build_local_metric_impl();
                quote! {
                    #code_struct
                    #code_impl
                    #code_trait_impl
                }
            })
            .collect();

        let scope_id = SCOPE_ID.fetch_add(1, Ordering::Relaxed);
        let scope_name = Ident::new(
            &format!("prometheus_static_scope_{}", scope_id),
            Span::call_site(),
        );

        let visibility = &metric.visibility;
        let struct_name = &metric.struct_name;

        quote! {
            #visibility use self::#scope_name::#struct_name;

            #[allow(dead_code)]
            mod #scope_name {
                use ::std::collections::HashMap;
                use ::prometheus::*;
                use ::prometheus::local::*;

                #[allow(unused_imports)]
                use super::*;

                #(
                    #label_struct
                )*
            }
        }
    }

    pub fn build_label_enum(label_enum: &MetricEnumDef) -> Tokens {
        let visibility = &label_enum.visibility;
        let enum_name = &label_enum.enum_name;
        let enum_item_names = label_enum.definitions.get_names();
        let enum_item_values = label_enum.definitions.get_values();
        let match_patterns = label_enum.build_fields_with_path();

        quote! {
            #[allow(dead_code)]
            #[allow(non_camel_case_types)]
            #[derive(Clone, Copy, PartialEq, Eq, Hash)]
            #visibility enum #enum_name {
                #(#enum_item_names),*
            }

            impl #enum_name {
                pub fn get_str(&self) -> &'static str {
                    match self {
                        #(
                            #match_patterns => #enum_item_values,
                        )*
                    }
                }
            }

            impl ::std::fmt::Debug for #enum_name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    write!(f, "{}", self.get_str())
                }
            }

            impl ::std::fmt::Display for #enum_name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    ::std::fmt::Debug::fmt(self, f)
                }
            }
        }
    }
}

struct MetricBuilderContext<'a> {
    metric: &'a MetricDef,
    enum_definitions: &'a HashMap<Ident, MetricEnumDef>,
    label: &'a MetricLabelDef,
    label_index: usize,
    is_last_label: bool,

    struct_name: Ident,
    member_type: Ident,
}

impl<'a> MetricBuilderContext<'a> {
    fn new(
        metric: &'a MetricDef,
        enum_definitions: &'a HashMap<Ident, MetricEnumDef>,
        label_index: usize,
    ) -> MetricBuilderContext<'a> {
        let is_last_label = label_index == metric.labels.len() - 1;
        MetricBuilderContext {
            metric,
            enum_definitions,
            label: &metric.labels[label_index],
            label_index,
            is_last_label,

            struct_name: util::get_label_struct_name(metric.struct_name.clone(), label_index),
            member_type: util::get_member_type(
                metric.struct_name.clone(),
                label_index,
                metric.metric_type.clone(),
                is_last_label,
            ),
        }
    }

    fn build_struct(&self) -> Tokens {
        let struct_name = &self.struct_name;

        let field_names = self
            .label
            .get_value_def_list(self.enum_definitions)
            .get_names();
        let member_types: Vec<_> = field_names.iter().map(|_| &self.member_type).collect();

        quote! {
            #[allow(missing_copy_implementations)]
            pub(super) struct #struct_name {
                #(
                    pub #field_names: #member_types,
                )*
            }
        }
    }

    fn build_impl(&self) -> Tokens {
        let struct_name = &self.struct_name;
        let impl_from = self.build_impl_from();
        let impl_get = self.build_impl_get();
        let impl_try_get = self.build_impl_try_get();
        let impl_flush = self.build_impl_flush();
        quote! {
            impl #struct_name {
                #impl_from
                #impl_get
                #impl_try_get
                #impl_flush
            }
        }
    }

    fn build_local_metric_impl(&self) -> Tokens {
        let struct_name = &self.struct_name;

        if util::is_local_metric(self.metric.metric_type.clone()) {
            quote! {
                impl ::prometheus::local::LocalMetric for #struct_name {
                    fn flush(&self) {
                        #struct_name::flush(self);
                    }
                }
            }
        } else {
            Tokens::new()
        }
    }

    fn build_impl_from(&self) -> Tokens {
        let struct_name = &self.struct_name;
        let metric_vec_type = util::to_non_local_metric_type(util::get_metric_vec_type(
            self.metric.metric_type.clone(),
        ));

        let prev_labels_ident: Vec<_> = (0..self.label_index)
            .map(|i| Ident::new(&format!("label_{}", i), Span::call_site()))
            .collect();
        let body = self.build_impl_from_body(&prev_labels_ident);

        quote! {
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

    fn build_impl_from_body(&self, prev_labels_ident: &[Ident]) -> Tokens {
        let member_type = &self.member_type;
        let bodies: Vec<_> = self
            .label
            .get_value_def_list(self.enum_definitions)
            .get()
            .iter()
            .map(|value| {
                let name = &value.name;
                let value = &value.value;
                if self.is_last_label {
                    let current_label = &self.label.label_key;
                    let prev_labels_str: Vec<_> = prev_labels_ident
                        .iter()
                        .enumerate()
                        .map(|(i, _)| &self.metric.labels[i].label_key)
                        .collect();
                    let local_suffix_call =
                        if util::is_local_metric(self.metric.metric_type.clone()) {
                            quote! { .local() }
                        } else {
                            Tokens::new()
                        };
                    quote! {
                        #name: m.with(&{
                            let mut coll = HashMap::new();
                            #(
                                coll.insert(#prev_labels_str, #prev_labels_ident);
                            )*
                            coll.insert(#current_label, #value);
                            coll
                        })#local_suffix_call,
                    }
                } else {
                    let prev_labels_ident = prev_labels_ident;
                    quote! {
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
        quote! {
            #(
                #bodies
            )*
        }
    }

    /// `fn get()` is only available when label is defined by `label_enum`.
    fn build_impl_get(&self) -> Tokens {
        let enum_ident = self.label.get_enum_ident();
        if let Some(e) = enum_ident {
            let member_type = &self.member_type;
            let match_patterns = self
                .enum_definitions
                .get(e)
                .unwrap()
                .build_fields_with_path();
            let fields = self
                .label
                .get_value_def_list(self.enum_definitions)
                .get_names();
            quote! {
                pub fn get(&self, value: #e) -> &#member_type {
                    match value {
                        #(
                            #match_patterns => &self.#fields,
                        )*
                    }
                }
            }
        } else {
            Tokens::new()
        }
    }

    fn build_impl_try_get(&self) -> Tokens {
        let member_type = &self.member_type;
        let value_def_list = self.label.get_value_def_list(self.enum_definitions);
        let names = value_def_list.get_names();
        let values = value_def_list.get_values();
        quote! {
            pub fn try_get(&self, value: &str) -> Option<&#member_type> {
                match value {
                    #(
                        #values => Some(&self.#names),
                    )*
                    _ => None,
                }
            }
        }
    }

    fn build_impl_flush(&self) -> Tokens {
        if util::is_local_metric(self.metric.metric_type.clone()) {
            let value_def_list = self.label.get_value_def_list(self.enum_definitions);
            let names = value_def_list.get_names();
            quote! {
                pub fn flush(&self) {
                    #(self.#names.flush();)*
                }
            }
        } else {
            Tokens::new()
        }
    }
}
