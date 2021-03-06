/*
 * Copyright (c) 2018 Boucher, Antoni <bouanto@zoho.com>
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the "Software"), to deal in
 * the Software without restriction, including without limitation the rights to
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
 * the Software, and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
 * FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
 * COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
 * IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

use proc_macro2::{Span,TokenStream};
use syn::{
    Expr,
    ExprLit,
    Ident,
    IntSuffix,
    Lit,
    LitInt,
};
use syn::spanned::Spanned;

use ast::QueryType;
use super::BackendGen;
use SqlQueryWithArgs;

pub struct PostgresBackend {}

pub fn create_backend() -> PostgresBackend {
    PostgresBackend {
    }
}

impl BackendGen for PostgresBackend {
    fn convert_index(&self, index: usize) -> TokenStream {
        quote! {
            #index
        }
    }

    fn delta_type(&self) -> TokenStream {
        quote! { usize }
    }

    /// Generate the Rust code using the `postgres` library depending on the `QueryType`.
    fn gen_query_expr(&self, connection_expr: TokenStream, args: &SqlQueryWithArgs, args_expr: TokenStream, struct_expr: TokenStream,
                      aggregate_struct: TokenStream, aggregate_expr: TokenStream) -> TokenStream
    {
        let result_ident = Ident::new("__tql_result", proc_macro2::Span::call_site());
        let sql_query = &args.sql;
        let std_ident = quote_spanned! { connection_expr.span() =>
            ::std
        };

        match args.query_type {
            QueryType::AggregateMulti => {
                quote! {{
                    #aggregate_struct
                    #connection_expr.prepare(#sql_query)
                        .and_then(|#result_ident| {
                            Ok(#result_ident.query(&#args_expr)?.iter()
                                .map(|__tql_item_row| {
                                    #aggregate_expr
                                }).collect::<Vec<_>>())
                                // TODO: return an iterator instead of a vector.
                        })
                }}
            },
            QueryType::AggregateOne => {
                quote! {{
                    #aggregate_struct
                    #connection_expr.prepare(#sql_query)
                        .and_then(|#result_ident| {
                            #result_ident.query(&#args_expr)?
                                .iter().next()
                                .ok_or_else(|| #std_ident::io::Error::from(#std_ident::io::ErrorKind::NotFound).into())
                                .map(|__tql_item_row| {
                                    #aggregate_expr
                                })
                        })
                }}
            },
            QueryType::Create => {
                quote! {
                    #connection_expr.prepare(#sql_query)
                        .and_then(|result| result.execute(&[]))
                }
            },
            QueryType::InsertOne => {
                quote! {
                    #connection_expr.prepare(#sql_query)
                        .and_then(|result| {
                            let rows = result.query(&#args_expr)?;
                            let __tql_item_row = rows.iter().next()
                                .ok_or_else(|| #std_ident::io::Error::from(#std_ident::io::ErrorKind::NotFound))?;
                            let count: i32 = __tql_item_row.get(0);
                            Ok(count)
                        })
                }
            },
            QueryType::SelectMulti => {
                quote! {
                    #connection_expr.prepare(#sql_query)
                        .and_then(|#result_ident| {
                            let #result_ident = #result_ident.query(&#args_expr)?;
                            let #result_ident = #result_ident.iter();
                            Ok(#result_ident.map(|__tql_item_row| {
                                #struct_expr
                            }).collect::<Vec<_>>())
                            // TODO: return an iterator instead of a vector.
                        })
                }
            },
            QueryType::SelectOne => {
                quote! {
                    #connection_expr.prepare(#sql_query)
                        .and_then(|#result_ident| {
                            let #result_ident = #result_ident.query(&#args_expr)?;
                            let __tql_item_row = #result_ident.iter().next()
                                .ok_or_else(|| #std_ident::io::Error::from(#std_ident::io::ErrorKind::NotFound))?;
                            Ok(#struct_expr)
                        })
                }
            },
            QueryType::Exec => {
                quote! {
                    #connection_expr.prepare(#sql_query)
                        .and_then(|result| result.execute(&#args_expr))
                }
            },
        }
    }

    fn int_literal(&self, num: usize) -> Expr {
        Expr::Lit(ExprLit {
            attrs: vec![],
            lit: Lit::Int(LitInt::new(num as u64, IntSuffix::Usize, Span::call_site())),
        })
    }

    fn row_type_ident(&self, table_ident: &Ident) -> proc_macro2::TokenStream {
        quote_spanned! { table_ident.span() =>
            ::postgres::rows::Row
        }
    }

    fn to_sql(&self, primary_key_ident: &Ident) -> proc_macro2::TokenStream {
        quote! {
            self.#primary_key_ident.to_sql(ty, out)
        }
    }

    fn to_sql_impl(&self, table_ident: &Ident, to_sql_code: proc_macro2::TokenStream) -> TokenStream {
        let std_ident = quote_spanned! { table_ident.span() =>
            ::std
        };
        let postgres_ident = quote_spanned! { table_ident.span() =>
            ::postgres
        };
        quote! {
            impl #postgres_ident::types::ToSql for #table_ident {
                fn to_sql(&self, ty: &#postgres_ident::types::Type, out: &mut Vec<u8>) ->
                    Result<#postgres_ident::types::IsNull, Box<#std_ident::error::Error + 'static + Sync + Send>>
                {
                    #to_sql_code
                }

                fn accepts(ty: &#postgres_ident::types::Type) -> bool {
                    *ty == #postgres_ident::types::INT4
                }

                fn to_sql_checked(&self, ty: &#postgres_ident::types::Type, out: &mut #std_ident::vec::Vec<u8>)
                    -> #std_ident::result::Result<#postgres_ident::types::IsNull,
                    Box<#std_ident::error::Error + #std_ident::marker::Sync + #std_ident::marker::Send>>
                {
                    #postgres_ident::types::__to_sql_checked(self, ty, out)
                }
            }
        }
    }
}
