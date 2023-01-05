//! A derive macro for creating **push-based** reactive properties for structs (with named fields only).
//!
//! # Why push-based?
//! Lazy *poll-based* reactive systems typically require wrapping the values and adding RefCells or flags to cache and update values. Event-based system require a subscription model.
//!
//! The plumbing for adding *push-based* change propagation is done via macros at compile-time and the generated code can be inlined during compilation, becoming a zero-cost abstraction at run-time (same as re-calculating the dynamic properties by hand when their dependencies change)
//!
//! The types can also be left untouched, no need for wrapping and dereferencing.
//!
//! # How to use
//! ### 1. Add as a dependency to the Cargo file
//! ```toml
//! [dependencies]
//! dynamic-struct = "*"
//! ```
//!
//! ## 2. Add the derive macro to the struct and mark the properties that are dynamic
//! ```ignore
//! use dynamic_struct::Dynamic;
//!
//! #[derive(Dynamic)]
//! struct Demo {
//!     a: u32,
//!     b: u32,
//!     #[dynamic((a, b), calculate_c)]
//!     c: u32,
//! }
//!
//! impl Demo {
//!     fn calculate_c(&mut self) {
//!         self.c = self.a + self.b
//!     }
//! }
//! ```
//!
//! The attribute for the properties has the following structure:
//! ```ignore
//! #[dynamic(tuple of dependent property names, name of local method name)]
//! ```
//!
//! The local method must have the call signature matching `fn name(&mut self)`.
//!
//! ## 3. Update the properties using the generated mutate functions
//! ```ignore
//! let demo = Demo { a: 1, b: 2, c: 3 };
//!
//! dbg!(demo.c); //3
//! demo.update_a(7);
//! dbg!(demo.c); //9
//! ```
//!
//! # How it works
//!
//! ## 1. Functions are created to signal when a property is changed, it is populated with the methods that should be called.
//!
//! ```ignore
//! impl Demo {
//!     #[inline]
//!     pub fn updated_a(&mut self) {
//!         self.update_c();
//!     }
//! }
//! ```
//!
//! Note: properties that do not propagate changes will still be created but will be empty.
//!
//! ## 2. Functions are created for each property to update the property
//!
//! For **non-dynamic** properties, the value can be set via a parameter matching the field type, then the field updated function is called (listed above).
//!
//! ```ignore
//! impl Demo {
//!     #[inline]
//!     pub fn update_a(&mut self, a: u32) {
//!         self.a = a;
//!         self.updated_a();
//!     }
//! }
//! ```
//!
//! For **dynamic** properties, the value is set by calling the specified dynamic function, then the field updated function is called (listed above).
//!
//! ```ignore
//! impl Demo {
//!     #[inline]
//!     pub fn update_c(&mut self) {
//!         self.calculate_c();
//!         self.updated_c();
//!     }
//! }
//! ```
//!
//! Note: be careful not to create cyclic dependencies!

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Data, DeriveInput, Fields, Ident, Token,
};

struct DynamicField {
    _paren_token: token::Paren,
    dependencies: Punctuated<Ident, Token![,]>,
    _comma: Token![,],
    method_name: Ident,
}

impl Parse for DynamicField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        Ok(DynamicField {
            _paren_token: parenthesized!(content in input),
            dependencies: content.parse_terminated(Ident::parse)?,
            _comma: input.parse()?,
            method_name: input.parse()?,
        })
    }
}

const DYNAMIC_ATTR_NAME: &str = "dynamic";

//TODO: make customisable per usage
const UPDATED_METHOD_PREFIX: &str = "updated_";
const UPDATED_METHOD_SUFFIX: &str = "";

const UPDATE_METHOD_PREFIX: &str = "update_";
const UPDATE_METHOD_SUFFIX: &str = "";

const SETTER_METHOD_PREFIX: &str = "update_";
const SETTER_METHOD_SUFFIX: &str = "";

fn create_ident(ident: &Ident, prefix: &str, suffix: &str) -> Ident {
    format_ident!("{}{}{}", prefix, ident, suffix)
}

fn create_updated_ident(ident: &Ident) -> Ident {
    create_ident(ident, UPDATED_METHOD_PREFIX, UPDATED_METHOD_SUFFIX)
}

fn create_setter_ident(ident: &Ident) -> Ident {
    create_ident(ident, SETTER_METHOD_PREFIX, SETTER_METHOD_SUFFIX)
}

fn create_update_ident(ident: &Ident) -> Ident {
    create_ident(ident, UPDATE_METHOD_PREFIX, UPDATE_METHOD_SUFFIX)
}

#[proc_macro_derive(Dynamic, attributes(dynamic))]
pub fn derive_dynamic(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, data, .. } = parse_macro_input!(input);

    //parse and merge the dynamic attribute for the struct

    //validate the usage of this macro and extract the field attributes
    let fields = match data {
        Data::Struct(data_struct) => match data_struct.fields {
            Fields::Named(fields) => fields.named,
            _ => panic!("Only structs with named fields currently supported!"),
        },
        _ => panic!("Only structs currently supported!"),
    };

    //parse the field 'dynamic' attributes
    let (dynamic_fields, non_dynamic_fields): (Vec<_>, Vec<_>) = fields
        .iter()
        .map(|field| {
            //merge the attributes that are marked as dynamic for the field
            let dynamic = field
                .attrs
                .iter()
                .find(|attr| {
                    attr.path
                        .get_ident()
                        .filter(|item| *item == DYNAMIC_ATTR_NAME)
                        .is_some()
                })
                .map(|attr| {
                    attr.parse_args::<DynamicField>()
                        .expect("Dynamic attribute format is invalid")
                });

            (field, dynamic)
        })
        .partition(|(_, dynamic)| dynamic.is_some());

    //create a list of vars to update based on the dependencies
    let mut inv_map: HashMap<&Ident, HashSet<&Ident>> = HashMap::new();

    dynamic_fields.iter().for_each(|(field, dynamic)| {
        let field_name = field.ident.as_ref().unwrap();

        dynamic
            .as_ref()
            .unwrap()
            .dependencies
            .iter()
            .for_each(|dependency| {
                inv_map
                    .entry(dependency)
                    .and_modify(|impacts| {
                        impacts.insert(field_name);
                    })
                    .or_insert_with(|| HashSet::from([field_name]));
            });
    });

    //updated methods based on the dependencies
    let updated_methods = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let func_name = create_updated_ident(field_name);
        let deps = inv_map
            .remove(field_name)
            .unwrap_or_default()
            .into_iter()
            .map(create_update_ident);

        quote! {
            #[inline]
            pub fn #func_name(&mut self) {
                #(
                    self.#deps();
                )*
            }
        }
    });

    //setters functions for non-dynamic functions that trigger the change functions
    let setter_methods = non_dynamic_fields.iter().map(|(field, _)| {
        let field_name = field.ident.as_ref().unwrap();
        let func_name = create_setter_ident(field_name);
        let updated_func_name = create_updated_ident(field_name);
        let typ = &field.ty;

        quote! {
            #[inline]
            pub fn #func_name(&mut self, value: #typ) {
                self.#field_name = value;
                self.#updated_func_name();
            }
        }
    });

    //update methods for dynamics (calls our desired function)
    let update_methods = dynamic_fields.iter().map(|(field, dynamic)| {
        let field_name = field.ident.as_ref().unwrap();
        let func_name = create_update_ident(field_name);
        let updated_func_name = create_updated_ident(field_name);
        let callable_name = &dynamic.as_ref().unwrap().method_name;

        quote! {
            #[inline]
            pub fn #func_name(&mut self) {
                self.#callable_name();
                self.#updated_func_name();
            }
        }
    });

    let output = quote! {
        impl #ident {
            #(
                #updated_methods
            )*
            #(
                #setter_methods
            )*
            #(
                #update_methods
            )*
        }
    };

    output.into()
}
