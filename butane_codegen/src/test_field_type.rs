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
    let _output = derive_field_type_impl(derive_input);
}
