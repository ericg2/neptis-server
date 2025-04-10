use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, DeriveInput, Ident, ItemFn, ItemImpl, ItemStruct, Meta, MetaList, ReturnType, Type};
use if_chain::if_chain;

/// Extracts the base type from the `#[web_dto(BaseType)]` attribute
fn extract_base_type(attrs: &[Attribute]) -> Option<Ident> {
    for attr in attrs {
        if attr.path().is_ident("web_dto") {
            // Parse the attribute arguments directly as an identifier.
            if let Ok(base) = attr.parse_args::<Ident>() {
                return Some(base);
            }
        }
    }
    None
}

#[proc_macro_derive(WebDto, attributes(web_dto))]
pub fn derive_web_dto(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let dto_name = &input.ident;
    let base_type = extract_base_type(&input.attrs)
        .expect("Missing #[web_dto(BaseType)] attribute");

    let expanded = quote! {
        impl crate::api::traits::WebDtoFrom<Vec<#base_type>> for Vec<#dto_name> {
            fn try_to_dto(auth_user: &User, item: Vec<#base_type>) -> Result<Self, crate::api::errors::NeptisError>
            where
                Self: serde::Serialize + Sized,
            {
                let mut output = Vec::new();
                for x in item {
                    output.push(#dto_name::try_to_dto(auth_user, x)?);
                }
                Ok(output)
            }
        }
    };

    TokenStream::from(expanded)
}
// WORKING 3-30-25
#[proc_macro_attribute]
pub fn action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident.clone();
    let async_name = syn::Ident::new(&format!("{}_async", name), name.span());
    // If no attribute is provided, default to `usize`

    let ret_type = match &input.sig.output {
        ReturnType::Type(_, ty) => ty.clone(),
        _ => {
            return syn::Error::new_spanned(
                &input.sig.ident,
                "Function must return Result<T, NeptisError>",
            )
            .to_compile_error()
            .into();
        }
    };

    // Ensure return type is Result<T, NeptisError>
    let valid_return_type = match &*ret_type {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last();
            last_segment.map_or(false, |segment| segment.ident == "Result")
        }
        _ => false,
    };

    if !valid_return_type {
        return syn::Error::new_spanned(
            &ret_type,
            "Function must return Result<T, NeptisError>",
        )
        .to_compile_error()
        .into();
    }

    // Extract the inner `T` type from `Result<T, NeptisError>`
    let result_type = if_chain! {
        if let syn::Type::Path(type_path) = &*ret_type;
        if let Some(segment) = type_path.path.segments.last();
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments;
        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first();
        then {
            inner_ty.clone()
        } 
        else {
            return syn::Error::new_spanned(
                &ret_type,
                "Invalid Result type",
            )
            .to_compile_error()
            .into();
        }
    };
   
    // Handle function arguments correctly
    let inputs = input.sig.inputs.iter().map(|arg| quote! { #arg }).collect::<Vec<_>>();
    let block = &input.block;

    let target_type: syn::Type = if attr.is_empty() {
        result_type.clone()
    } else {
        syn::parse_macro_input!(attr as syn::Type)
    };


    let expanded = quote! {
        pub async fn #async_name(
            conn: &mut AsyncPgConnection,
            auth_user: &User,
            #(#inputs),*
        ) -> Result<#result_type, NeptisError>
        where
            #result_type: crate::api::traits::WebDtoFrom<#target_type>
        {
            let res: Result<#target_type, NeptisError> = { #block };
            crate::to_dto!(#target_type, #result_type, auth_user, res?)
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn no_auth_action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident.clone();
    let async_name = syn::Ident::new(&format!("{}_async", name), name.span());
    // If no attribute is provided, default to `usize`

    let ret_type = match &input.sig.output {
        ReturnType::Type(_, ty) => ty.clone(),
        _ => {
            return syn::Error::new_spanned(
                &input.sig.ident,
                "Function must return Result<T, NeptisError>",
            )
            .to_compile_error()
            .into();
        }
    };

    // Ensure return type is Result<T, NeptisError>
    let valid_return_type = match &*ret_type {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last();
            last_segment.map_or(false, |segment| segment.ident == "Result")
        }
        _ => false,
    };

    if !valid_return_type {
        return syn::Error::new_spanned(
            &ret_type,
            "Function must return Result<T, NeptisError>",
        )
        .to_compile_error()
        .into();
    }

    // Extract the inner `T` type from `Result<T, NeptisError>`
    let result_type = if_chain! {
        if let syn::Type::Path(type_path) = &*ret_type;
        if let Some(segment) = type_path.path.segments.last();
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments;
        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first();
        then {
            inner_ty.clone()
        } 
        else {
            return syn::Error::new_spanned(
                &ret_type,
                "Invalid Result type",
            )
            .to_compile_error()
            .into();
        }
    };
   
    // Handle function arguments correctly
    let inputs = input.sig.inputs.iter().map(|arg| quote! { #arg }).collect::<Vec<_>>();
    let block = &input.block;

    let target_type: syn::Type = if attr.is_empty() {
        result_type.clone()
    } else {
        syn::parse_macro_input!(attr as syn::Type)
    };


    let expanded = quote! {
        pub async fn #async_name(
            conn: &mut AsyncPgConnection,
            #(#inputs),*
        ) -> Result<#result_type, NeptisError>
        where
            #result_type: crate::api::traits::WebDtoFrom<#target_type>
        {
            #block
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn admin_action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident.clone();
    let async_name = syn::Ident::new(&format!("priv_{}_async", name), name.span());
    // If no attribute is provided, default to `usize`

    let ret_type = match &input.sig.output {
        ReturnType::Type(_, ty) => ty.clone(),
        _ => {
            return syn::Error::new_spanned(
                &input.sig.ident,
                "Function must return Result<T, NeptisError>",
            )
            .to_compile_error()
            .into();
        }
    };

    // Ensure return type is Result<T, NeptisError>
    let valid_return_type = match &*ret_type {
        syn::Type::Path(type_path) => {
            let last_segment = type_path.path.segments.last();
            last_segment.map_or(false, |segment| segment.ident == "Result")
        }
        _ => false,
    };

    if !valid_return_type {
        return syn::Error::new_spanned(
            &ret_type,
            "Function must return Result<T, NeptisError>",
        )
        .to_compile_error()
        .into();
    }

    // Extract the inner `T` type from `Result<T, NeptisError>`
    let result_type = if_chain! {
        if let syn::Type::Path(type_path) = &*ret_type;
        if let Some(segment) = type_path.path.segments.last();
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments;
        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first();
        then {
            inner_ty.clone()
        } 
        else {
            return syn::Error::new_spanned(
                &ret_type,
                "Invalid Result type",
            )
            .to_compile_error()
            .into();
        }
    };
   
    // Handle function arguments correctly
    let inputs = input.sig.inputs.iter().map(|arg| quote! { #arg }).collect::<Vec<_>>();
    let block = &input.block;

    let target_type: syn::Type = if attr.is_empty() {
        result_type.clone()
    } else {
        syn::parse_macro_input!(attr as syn::Type)
    };


    let expanded = quote! {
        pub async fn #async_name(
            conn: &mut AsyncPgConnection,
            auth_user: &User,
            #(#inputs),*
        ) -> Result<#result_type, NeptisError>
        where
            #result_type: crate::api::traits::WebDtoFrom<#target_type>
        {
            let res: Result<#target_type, NeptisError> = { 
                if !auth_user.is_admin {
                    return Err(NeptisError::Unauthorized(
                        "You must be admin to create a user!".to_string(),
                    ));
                }
                #block 
            };
            crate::to_dto!(#target_type, #result_type, auth_user, res?)
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident.clone();
    let async_name = syn::Ident::new(&format!("{}", name), name.span());

    let ret_type = match &input.sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        _ => quote! { () }, // Default to () if no return type
    };

    // Extract arguments
    let inputs = input.sig.inputs.iter();
    let remaining_params: Vec<_> = inputs.collect(); // Keep all parameters
    let body = &input.block;

    let expanded = quote! {
        async fn #async_name(
            _user: User, 
            mut db: Connection<Db>,
            #(#remaining_params),*
        ) -> #ret_type {
            let conn = &mut **db;
            let auth_user = &_user;
            #body
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn no_auth_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = input.sig.ident.clone();
    let async_name = syn::Ident::new(&format!("handle_{}", name), name.span());

    let ret_type = match &input.sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        _ => quote! { () }, // Default to () if no return type
    };

    // Extract arguments
    let inputs = input.sig.inputs.iter();
    let remaining_params: Vec<_> = inputs.collect(); // Keep all parameters
    let body = &input.block;

    let expanded = quote! {
        async fn #async_name(
            mut db: Connection<Db>,
            #(#remaining_params),*
        ) -> #ret_type {
            let conn = &mut **db;
            #body
        }
    };

    TokenStream::from(expanded)
}