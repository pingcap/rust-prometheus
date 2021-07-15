// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use proc_macro2::Span;
use syn::Ident;

pub fn is_local_metric(metric_type: Ident) -> bool {
    metric_type.to_string().starts_with("Local")
}

pub fn to_non_local_metric_type(metric_type: Ident) -> Ident {
    let metric_type_str = metric_type.to_string();
    if let Some(stripped) = metric_type_str.strip_prefix("Local") {
        Ident::new(stripped, Span::call_site())
    } else {
        metric_type
    }
}

pub fn get_metric_vec_type(metric_type: Ident) -> Ident {
    Ident::new(&format!("{}Vec", metric_type), Span::call_site())
}

pub fn get_label_struct_name(struct_name: Ident, label_index: usize) -> Ident {
    let mut struct_name = struct_name.to_string();
    if label_index > 0 {
        struct_name.push_str(&(label_index + 1).to_string());
    }
    Ident::new(&struct_name, Span::call_site())
}

pub fn get_member_type(
    struct_name: Ident,
    label_index: usize,
    metric_type: Ident,
    is_last_label: bool,
) -> Ident {
    if is_last_label {
        metric_type
    } else {
        get_label_struct_name(struct_name, label_index + 1)
    }
}

pub fn get_inner_member_type(
    struct_name: Ident,
    label_index: usize,
    metric_type: Ident,
    is_last_label: bool,
) -> Ident {
    if is_last_label {
        metric_type
    } else {
        Ident::new(
            &format!(
                "{}Inner",
                get_label_struct_name(struct_name, label_index + 1)
            ),
            Span::call_site(),
        )
    }
}

pub fn get_delegator_member_type(
    struct_name: Ident,
    label_index: usize,
    is_last_label: bool,
) -> Ident {
    if is_last_label {
        Ident::new("usize", Span::call_site())
    } else {
        Ident::new(
            &format!(
                "{}{}",
                get_label_struct_name(struct_name, label_index + 1),
                "Delegator"
            ),
            Span::call_site(),
        )
    }
}
