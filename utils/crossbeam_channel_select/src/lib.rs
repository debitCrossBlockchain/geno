extern crate proc_macro;

use self::proc_macro::TokenStream;

use proc_macro2::Span;
use quote::quote;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, Token,
};

mod kw {
    syn::custom_keyword!(recv);
    syn::custom_keyword!(recv_any);
    syn::custom_keyword!(send);
    syn::custom_keyword!(send_any);
}

enum SelectType {
    Recv {
        receiver: Expr,
        result: Ident,
        body: Expr,
    },
    RecvAny {
        receiver: Expr,
        result: Ident,
        index: Ident,
        body: Expr,
    },
    Send {
        sender: Expr,
        message: Expr,
        result: Ident,
        body: Expr,
    },
    SendAny {
        sender: Expr,
        message: Expr,
        result: Ident,
        index: Ident,
        body: Expr,
    },
}

/// Parse tokens matching `(expr) ->`
fn parse_recv_params(input: &ParseStream) -> Result<Expr> {
    let content;
    let _ = parenthesized!(content in input);
    let receiver: Expr = content.parse()?;
    input.parse::<Token![->]>()?;

    Ok(receiver)
}

/// Parse tokens matching `(expr, expr) ->`
fn parse_send_params(input: &ParseStream) -> Result<(Expr, Expr)> {
    let content;
    let _ = parenthesized!(content in input);
    let mut params: Punctuated<Expr, Token![,]> = content.parse_terminated(Expr::parse)?;

    // TODO: Better error reporting, the span could point to a more specific location
    let sender = params
        .pop()
        .ok_or_else(|| content.error("Expected (sender: expr, message: expr)"))?
        .into_value();
    let message = params
        .pop()
        .ok_or_else(|| content.error("Expected (sender: expr, message: expr)"))?
        .into_value();

    input.parse::<Token![->]>()?;

    Ok((sender, message))
}

/// Parse tokens matching `ident => expr`
fn parse_result(input: &ParseStream) -> Result<(Ident, Expr)> {
    // TODO: handle `_` as indent
    // TODO: allow the result ident to be optional

    let result: Ident = input.parse::<Ident>()?;
    input.parse::<Token![=>]>()?;
    let body: Expr = input.parse::<Expr>()?;

    Ok((result, body))
}

/// Parse tokens matching `ident, ident => expr`
fn parse_any_result(input: &ParseStream) -> Result<(Ident, Ident, Expr)> {
    // TODO: handle `_` as indent
    // TODO: allow the result ident and the index ident to be optional

    let result: Ident = input.parse::<Ident>()?;
    input.parse::<Token![,]>()?;
    let index: Ident = input.parse::<Ident>()?;
    input.parse::<Token![=>]>()?;
    let body: Expr = input.parse::<Expr>()?;

    Ok((result, index, body))
}

fn parse_selection(input: &ParseStream) -> Result<SelectType> {
    let lookahead = input.lookahead1();
    if lookahead.peek(kw::recv) {
        input.parse::<kw::recv>()?;

        let receiver = parse_recv_params(input)?;
        let (result, body) = parse_result(input)?;

        Ok(SelectType::Recv {
            receiver,
            result,
            body,
        })
    } else if lookahead.peek(kw::recv_any) {
        input.parse::<kw::recv_any>()?;

        let receiver = parse_recv_params(input)?;
        let (result, index, body) = parse_any_result(input)?;

        Ok(SelectType::RecvAny {
            receiver,
            result,
            index,
            body,
        })
    } else if lookahead.peek(kw::send) {
        input.parse::<kw::send>()?;

        let (sender, message) = parse_send_params(input)?;
        let (result, body) = parse_result(input)?;

        Ok(SelectType::Send {
            sender,
            message,
            result,
            body,
        })
    } else if lookahead.peek(kw::send_any) {
        input.parse::<kw::send_any>()?;

        let (sender, message) = parse_send_params(input)?;
        let (result, index, body) = parse_any_result(input)?;

        Ok(SelectType::SendAny {
            sender,
            message,
            result,
            index,
            body,
        })
    } else {
        Err(lookahead.error())
    }
}

struct DynamicSelect {
    selections: Vec<SelectType>,
}

impl Parse for DynamicSelect {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut selections = vec![];

        while !input.is_empty() {
            selections.push(parse_selection(&input)?);

            // Parse optional `,` token after each entry
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(DynamicSelect { selections })
    }
}

#[proc_macro]
pub fn dynamic_select(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DynamicSelect);

    let mut selections = vec![];
    let mut matches = vec![];

    for (index, selection) in input.selections.into_iter().enumerate() {
        let id = Ident::new(&format!("__sel_{}", index), Span::call_site());

        match selection {
            SelectType::Recv {
                receiver,
                result,
                body,
            } => {
                selections.push(quote! {
                    let #id = __sel.recv(#receiver);
                    __sel_count += 1;
                });
                matches.push(quote! {
                    __i if __i == #id => {
                        let #result = __op.recv(#receiver);
                        #body
                    }
                });
            }

            SelectType::RecvAny {
                receiver,
                result,
                index,
                body,
            } => {
                let id_end = Ident::new(&format!("__sel_{}_end", index), Span::call_site());

                selections.push(quote! {
                    let #id = __sel_count;
                    let __receivers = #receiver;
                    for r in __receivers.iter() {
                        __sel.recv(r);
                        __sel_count += 1;
                    }

                    let #id_end = __sel_count;
                });

                matches.push(quote! {
                    __i if __i >= #id && __i < #id_end => {
                        let #index = __i - #id;
                        let #result = __op.recv(__receivers.iter().nth(#index).unwrap());
                        #body
                    }
                });
            }

            SelectType::Send {
                sender,
                message,
                result,
                body,
            } => {
                selections.push(quote! {
                    let #id = __sel.send(#sender);
                });
                matches.push(quote! {
                    __i if __i == #id => {
                        let #result = __op.send(#sender, #message);
                        #body
                    }
                });
            }

            SelectType::SendAny {
                sender,
                message,
                index,
                result,
                body,
            } => {
                let id_end = Ident::new(&format!("__sel_{}_end", index), Span::call_site());

                selections.push(quote! {
                    let #id = __sel_count;
                    let __senders = #sender;
                    for s in __senders.iter() {
                        __sel.send(s);
                        __sel_count += 1;
                    }

                    let #id_end = __sel_count;
                });

                matches.push(quote! {
                    __i if __i >= #id && __i < #id_end => {
                        let #index = __i - #id;
                        let #result = __op.send(__senders.iter().nth(#index).unwrap(), #message);
                        #body
                    }
                });
            }
        }
    }

    let expanded = quote! {
        let mut __sel = crossbeam_channel::Select::new();
        let mut __sel_count = 0;

        #(#selections)*

        let __op = __sel.select();
        match __op.index() {
            #(#matches),*
            _ => unreachable!(),
        }
    };

    TokenStream::from(expanded)
}
