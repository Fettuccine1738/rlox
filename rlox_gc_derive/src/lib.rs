use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(Trace, attributes(unsafe_ignore_trace))]
pub fn derive_trace(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let trace_body = match &input.data {
        Data::Struct(s) => trace_fields(&s.fields),
        Data::Enum(e) => {
            let arms = e.variants.iter().map(|variant| {
                let vname = &variant.ident;

                // Check for #[unsafe_ignore_trace] on the variant itself
                let variant_ignored = variant
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("unsafe_ignore_trace"));

                let fields = &variant.fields;
                let (pattern, trace) = if variant_ignored {
                    // Generate a wildcard pattern with empty trace body
                    match fields {
                        Fields::Named(_) => (quote! { #vname { .. } }, quote! {}),
                        Fields::Unnamed(_) => (quote! { #vname(..) }, quote! {}),
                        Fields::Unit => (quote! { #vname }, quote! {}),
                    }
                } else {
                    trace_enum_variant(vname, fields)
                };

                quote! { Self::#pattern => { #trace } }
            });
            quote! { match self { #(#arms,)* } }
        }
        Data::Union(_) => panic!("Trace cannot be derived for unions"),
    };

    quote! {
        impl #impl_generics crate::runtime::gc::Trace for #name #ty_generics #where_clause {
            fn trace(&self, heap: &mut crate::runtime::heap::Heap) {
                #trace_body
            }
        }
    }
    .into()
}

/// Generate trace calls for struct fields
fn trace_fields(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(f) => {
            let calls = f.named.iter().filter_map(|field| {
                // #[unsafe_ignore_trace] opts a field out (e.g. for marker types)
                let ignored = field
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("unsafe_ignore_trace"));
                if ignored {
                    return None;
                }
                let name = &field.ident;
                Some(quote! { Trace::trace(&self.#name, heap); })
            });
            quote! { #(#calls)* }
        }
        Fields::Unnamed(f) => {
            let calls = f.unnamed.iter().enumerate().filter_map(|(i, field)| {
                let ignored = field
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("unsafe_ignore_trace"));
                if ignored {
                    return None;
                }
                let idx = syn::Index::from(i);
                Some(quote! { Trace::trace(&self.#idx, heap); })
            });
            quote! { #(#calls)* }
        }
        Fields::Unit => quote! {},
    }
}

/// Generate a match arm pattern + trace body for an enum variant
fn trace_enum_variant(
    vname: &syn::Ident,
    fields: &Fields,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    match fields {
        Fields::Named(f) => {
            let names: Vec<_> = f.named.iter().map(|f| &f.ident).collect();
            let calls = f.named.iter().filter_map(|field| {
                let ignored = field
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("unsafe_ignore_trace"));
                if ignored {
                    return None;
                }
                let n = &field.ident;
                Some(quote! { Trace::trace(#n, heap); })
            });
            (quote! { #vname { #(#names,)* } }, quote! { #(#calls)* })
        }
        Fields::Unnamed(f) => {
            let bindings: Vec<_> = (0..f.unnamed.len())
                .map(|i| syn::Ident::new(&format!("__field{i}"), proc_macro2::Span::call_site()))
                .collect();
            let calls = f.unnamed.iter().enumerate().filter_map(|(i, field)| {
                let ignored = field
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("unsafe_ignore_trace"));
                if ignored {
                    return None;
                }
                let b = &bindings[i];
                Some(quote! { Trace::trace(#b, heap); })
            });
            (quote! { #vname(#(#bindings,)*) }, quote! { #(#calls)* })
        }
        Fields::Unit => (quote! { #vname }, quote! {}),
    }
}
