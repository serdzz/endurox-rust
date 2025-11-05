use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derive macro for automatic UbfStruct implementation
///
/// # Example
///
/// ```ignore
/// #[derive(UbfStruct)]
/// struct Transaction {
///     #[ubf(field = 1002)]
///     name: String,
///     
///     #[ubf(field = 1012)]
///     id: i64,
///     
///     #[ubf(field = 1021)]
///     amount: f64,
///     
///     #[ubf(field = 1004, default = "pending")]
///     status: String,
/// }
/// ```
#[proc_macro_derive(UbfStruct, attributes(ubf))]
pub fn derive_ubf_struct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    let name = &input.ident;
    
    // Parse struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("UbfStruct only supports named fields"),
        },
        _ => panic!("UbfStruct only supports structs"),
    };
    
    // Generate from_ubf implementation
    let mut from_ubf_fields = Vec::new();
    let mut to_ubf_fields = Vec::new();
    
    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        
        // Parse #[ubf(field = ...)] attribute
        let mut field_expr: Option<proc_macro2::TokenStream> = None;
        let mut default_value: Option<String> = None;
        
        for attr in &field.attrs {
            if attr.path().is_ident("ubf") {
                // Parse the meta list manually from tokens
                let tokens_str = attr.meta.require_list()
                    .expect("Expected meta list")
                    .tokens.to_string();
                
                // Split by comma and process each part
                for part in tokens_str.split(',') {
                    let part = part.trim();
                    
                    if part.starts_with("field") {
                        // Parse "field = <expr>" where expr can be a constant or literal
                        if let Some(eq_pos) = part.find('=') {
                            let value_str = part[eq_pos + 1..].trim();
                            // Store as token stream to support both literals and constants
                            field_expr = Some(value_str.parse().expect("Failed to parse field expression"));
                        }
                    } else if part.starts_with("default") {
                        // Parse "default = "value""
                        if let Some(eq_pos) = part.find('=') {
                            let value_str = part[eq_pos + 1..].trim();
                            default_value = Some(value_str.trim_matches('"').to_string());
                        }
                    }
                }
            }
        }
        
        let fid = field_expr.unwrap_or_else(|| panic!("Field {} must have #[ubf(field = ...)] attribute", field_name));
        
        // Generate field reading code based on type
        let field_getter = generate_field_getter(field_name, field_type, fid.clone(), default_value.as_deref());
        from_ubf_fields.push(field_getter);
        
        // Generate field writing code
        let field_setter = generate_field_setter(field_name, field_type, fid);
        to_ubf_fields.push(field_setter);
    }
    
    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    
    // Generate the implementation
    let expanded = quote! {
        impl ::endurox_sys::ubf_struct::UbfStruct for #name {
            fn from_ubf(buf: &::endurox_sys::ubf::UbfBuffer) -> Result<Self, ::endurox_sys::ubf_struct::UbfError> {
                #(#from_ubf_fields)*
                
                Ok(Self {
                    #(#field_names),*
                })
            }
            
            fn to_ubf(&self) -> Result<::endurox_sys::ubf::UbfBuffer, ::endurox_sys::ubf_struct::UbfError> {
                let mut buf = ::endurox_sys::ubf::UbfBuffer::new(2048)
                    .map_err(|e| ::endurox_sys::ubf_struct::UbfError::AllocationError(e))?;
                self.update_ubf(&mut buf)?;
                Ok(buf)
            }
            
            fn update_ubf(&self, buf: &mut ::endurox_sys::ubf::UbfBuffer) -> Result<(), ::endurox_sys::ubf_struct::UbfError> {
                #(#to_ubf_fields)*
                Ok(())
            }
        }
    };
    
    TokenStream::from(expanded)
}

fn generate_field_getter(
    field_name: &syn::Ident,
    field_type: &syn::Type,
    field_id: proc_macro2::TokenStream,
    default_value: Option<&str>,
) -> proc_macro2::TokenStream {
    let type_str = quote!(#field_type).to_string();
    
    if type_str.contains("String") {
        if let Some(default) = default_value {
            quote! {
                let #field_name = buf.get_string(#field_id, 0)
                    .unwrap_or_else(|_| #default.to_string());
            }
        } else {
            quote! {
                let #field_name = buf.get_string(#field_id, 0)
                    .map_err(|e| ::endurox_sys::ubf_struct::UbfError::FieldNotFound(
                        format!("Field {} ({}): {}", stringify!(#field_name), #field_id, e)
                    ))?;
            }
        }
    } else if type_str.contains("i64") || type_str.contains("i32") || type_str.contains("long") {
        quote! {
            let #field_name = buf.get_long(#field_id, 0)
                .map_err(|e| ::endurox_sys::ubf_struct::UbfError::FieldNotFound(
                    format!("Field {} ({}): {}", stringify!(#field_name), #field_id, e)
                ))? as #field_type;
        }
    } else if type_str.contains("f64") || type_str.contains("f32") || type_str.contains("double") {
        quote! {
            let #field_name = buf.get_double(#field_id, 0)
                .map_err(|e| ::endurox_sys::ubf_struct::UbfError::FieldNotFound(
                    format!("Field {} ({}): {}", stringify!(#field_name), #field_id, e)
                ))? as #field_type;
        }
    } else if type_str.contains("bool") {
        quote! {
            let #field_name = buf.is_present(#field_id, 0);
        }
    } else {
        // Assume it's a nested struct that implements UbfStruct
        quote! {
            let #field_name = <#field_type as ::endurox_sys::ubf_struct::UbfStruct>::from_ubf(buf)?;
        }
    }
}

fn generate_field_setter(
    field_name: &syn::Ident,
    field_type: &syn::Type,
    field_id: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let type_str = quote!(#field_type).to_string();
    
    if type_str.contains("String") {
        quote! {
            buf.add_string(#field_id, &self.#field_name)
                .map_err(|e| ::endurox_sys::ubf_struct::UbfError::TypeError(
                    format!("Field {}: {}", stringify!(#field_name), e)
                ))?;
        }
    } else if type_str.contains("i64") || type_str.contains("i32") || type_str.contains("long") {
        quote! {
            buf.add_long(#field_id, self.#field_name as i64)
                .map_err(|e| ::endurox_sys::ubf_struct::UbfError::TypeError(
                    format!("Field {}: {}", stringify!(#field_name), e)
                ))?;
        }
    } else if type_str.contains("f64") || type_str.contains("f32") || type_str.contains("double") {
        quote! {
            buf.add_double(#field_id, self.#field_name as f64)
                .map_err(|e| ::endurox_sys::ubf_struct::UbfError::TypeError(
                    format!("Field {}: {}", stringify!(#field_name), e)
                ))?;
        }
    } else if type_str.contains("bool") {
        quote! {
            if self.#field_name {
                buf.add_long(#field_id, 1)
                    .map_err(|e| ::endurox_sys::ubf_struct::UbfError::TypeError(
                        format!("Field {}: {}", stringify!(#field_name), e)
                    ))?;
            }
        }
    } else {
        // Assume it's a nested struct that implements UbfStruct
        quote! {
            self.#field_name.update_ubf(buf)?;
        }
    }
}
