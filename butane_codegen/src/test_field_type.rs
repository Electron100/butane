use quote::ToTokens;
use syn::{parse_quote, DeriveInput};

use super::derive_field_type_impl;

#[test]
fn r_hash_derive_field_type_newtype_json() {
    let item: syn::ItemStruct = parse_quote! {
        pub struct r#type {
            foo: String,
        }
    };

    let tokens = item.to_token_stream();
    let derive_input = syn::parse2::<DeriveInput>(tokens).unwrap();
    eprintln!("derive_input: {derive_input:?}");
    let output = derive_field_type_impl(derive_input);

    // Convert output to string for inspection
    let output_str = output.to_string();
    eprintln!("output: {output_str}");

    // Verify that the output contains the struct name with raw identifier
    assert!(
        output_str.contains("r#type"),
        "Output should contain 'r#type'"
    );

    // Verify that FieldType trait implementation is present
    assert!(
        output_str.contains("impl butane :: FieldType for r#type"),
        "Output should implement FieldType for r#type"
    );

    // Verify that the SqlType is Json (since it's a struct with named fields)
    assert!(
        output_str.contains("butane :: SqlType :: Json"),
        "Output should use SqlType::Json"
    );
}
