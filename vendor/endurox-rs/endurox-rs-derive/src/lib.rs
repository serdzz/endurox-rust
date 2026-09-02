use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, Fields, Ident, LitBool, Result, Type,
};

#[proc_macro_derive(UbfSerialize, attributes(ubf))]
pub fn derive_ubf_serialize(input: TokenStream) -> TokenStream {
    match expand_ubf_serialize(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

#[proc_macro_derive(UbfDeserialize, attributes(ubf))]
pub fn derive_ubf_deserialize(input: TokenStream) -> TokenStream {
    match expand_ubf_deserialize(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

#[derive(Default)]
struct UbfAttr {
    field: Option<Expr>,
    nested: bool,
    size: Option<Expr>,
    skip: bool,
}

struct MappedField {
    ident: Ident,
    ty: Type,
    attr: UbfAttr,
}

fn expand_ubf_serialize(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let fields = mapped_fields(&input.data)?;

    let writes = fields
        .iter()
        .filter(|field| !field.attr.skip)
        .map(|field| {
            let ident = &field.ident;
            let field_id = field_id(field)?;
            if field.attr.nested {
                let size = field
                    .attr
                    .size
                    .as_ref()
                    .map(ToTokens::to_token_stream)
                    .unwrap_or_else(|| quote!(1024));
                Ok(quote! {
                    ::endurox_rs::ubf_write_nested(
                        ubf,
                        #field_id,
                        0,
                        &self.#ident,
                        #size,
                        realloc,
                    )?;
                })
            } else {
                Ok(quote! {
                    ::endurox_rs::UbfFieldSerialize::ubf_write_field(
                        &self.#ident,
                        ubf,
                        #field_id,
                        0,
                        realloc,
                    )?;
                })
            }
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        impl #impl_generics ::endurox_rs::UbfSerialize for #name #ty_generics #where_clause {
            fn ubf_serialize<'ctx>(
                &self,
                ubf: &mut ::endurox_rs::TypedUbf<'ctx>,
                realloc: bool,
            ) -> ::endurox_rs::UbfResult<()> {
                #(#writes)*
                Ok(())
            }
        }
    })
}

fn expand_ubf_deserialize(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let fields = mapped_fields(&input.data)?;

    let reads = fields
        .iter()
        .filter(|field| !field.attr.skip)
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            let field_id = field_id(field)?;
            if field.attr.nested {
                Ok(quote! {
                    #ident: ::endurox_rs::ubf_read_nested(ubf, #field_id, 0)?,
                })
            } else {
                Ok(quote! {
                    #ident: <#ty as ::endurox_rs::UbfFieldDeserialize>::ubf_read_field(
                        ubf,
                        #field_id,
                        0,
                    )?,
                })
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let skipped = fields.iter().filter(|field| field.attr.skip).map(|field| {
        let ident = &field.ident;
        quote! { #ident: ::std::default::Default::default(), }
    });

    Ok(quote! {
        impl #impl_generics ::endurox_rs::UbfDeserialize for #name #ty_generics #where_clause {
            fn ubf_deserialize<'ctx>(
                ubf: &::endurox_rs::TypedUbf<'ctx>,
            ) -> ::endurox_rs::UbfResult<Self> {
                Ok(Self {
                    #(#reads)*
                    #(#skipped)*
                })
            }
        }
    })
}

fn mapped_fields(data: &Data) -> Result<Vec<MappedField>> {
    let Data::Struct(data) = data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "UbfSerialize/UbfDeserialize only support structs",
        ));
    };

    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "UbfSerialize/UbfDeserialize require named struct fields",
        ));
    };

    fields
        .named
        .iter()
        .map(|field| {
            let ident = field.ident.clone().ok_or_else(|| {
                syn::Error::new_spanned(field, "UbfSerialize/UbfDeserialize require named fields")
            })?;
            Ok(MappedField {
                ident,
                ty: field.ty.clone(),
                attr: parse_ubf_attr(&field.attrs)?,
            })
        })
        .collect()
}

fn field_id(field: &MappedField) -> Result<proc_macro2::TokenStream> {
    field
        .attr
        .field
        .as_ref()
        .map(ToTokens::to_token_stream)
        .ok_or_else(|| {
            syn::Error::new_spanned(
                &field.ident,
                "missing #[ubf(field = ...)] attribute for mapped UBF field",
            )
        })
}

fn parse_ubf_attr(attrs: &[Attribute]) -> Result<UbfAttr> {
    let mut result = UbfAttr::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("ubf")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("field") {
                result.field = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("nested") {
                result.nested = parse_optional_bool(meta)?;
                Ok(())
            } else if meta.path.is_ident("size") {
                result.size = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("skip") {
                result.skip = parse_optional_bool(meta)?;
                Ok(())
            } else {
                Err(meta.error("unsupported ubf attribute"))
            }
        })?;
    }

    Ok(result)
}

fn parse_optional_bool(meta: syn::meta::ParseNestedMeta<'_>) -> Result<bool> {
    if meta.input.peek(syn::Token![=]) {
        let value: LitBool = meta.value()?.parse()?;
        Ok(value.value)
    } else {
        Ok(true)
    }
}
