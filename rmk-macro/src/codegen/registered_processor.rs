use proc_macro2::TokenStream;
use quote::quote;
use rmk_config::{ChipModel, KeyboardTomlConfig, PinConfig};
use syn::ItemMod;

use super::chip::gpio::convert_gpio_str_to_output_pin;

/// Expand processor init/exec blocks from keyboard config.
/// Returns (initializers, executors).
pub(crate) fn expand_registered_processor_init(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> (TokenStream, Vec<TokenStream>) {
    let mut initializers = TokenStream::new();
    let mut executors = vec![];

    let (i, e) = expand_light_indicator_processors(keyboard_config);
    initializers.extend(i);
    executors.extend(e);

    // Custom processors declared in the module.
    if let Some((_, items)) = &item_mod.content {
        items.iter().for_each(|item| {
            if let syn::Item::Fn(item_fn) = &item
                && let Some(attr) = item_fn.attrs.iter().find(|attr| attr.path().is_ident("register_processor")) {
                    let _ = attr.parse_nested_meta(|meta| {
                        let (custom_init, custom_exec) = expand_custom_processor(item_fn);
                        initializers.extend(custom_init);

                        if meta.path.is_ident("event") || meta.path.is_ident("poll") {
                            // Processor runnables are executed through `run()`.
                            executors.push(quote! { #custom_exec.run() });
                            return Ok(());
                        }

                        panic!("#[register_processor] must specify execution mode with `event` or `poll`. Use `#[register_processor(event)]` or `#[register_processor(poll)]`")
                    });
                }
        });
    }

    (initializers, executors)
}

fn expand_light_indicator_processors(
    keyboard_config: &KeyboardTomlConfig,
) -> (TokenStream, Vec<TokenStream>) {
    let chip = keyboard_config.get_chip_model().unwrap();
    let light_config = keyboard_config.get_light_config();

    let mut initializers = TokenStream::new();
    let mut executors = vec![];

    create_keyboard_indicator_processor(
        &chip,
        &light_config.numslock,
        quote! { numlock_processor },
        quote! { NumLock },
        &mut initializers,
        &mut executors,
    );

    create_keyboard_indicator_processor(
        &chip,
        &light_config.scrolllock,
        quote! { scrolllock_processor },
        quote! { ScrollLock },
        &mut initializers,
        &mut executors,
    );

    create_keyboard_indicator_processor(
        &chip,
        &light_config.capslock,
        quote! { capslock_processor },
        quote! { CapsLock },
        &mut initializers,
        &mut executors,
    );

    (initializers, executors)
}

fn create_keyboard_indicator_processor(
    chip: &ChipModel,
    pin_config: &Option<PinConfig>,
    processor_ident: TokenStream,
    led_indicator_variant: TokenStream,
    initializers: &mut TokenStream,
    executors: &mut Vec<TokenStream>,
) {
    if let Some(c) = pin_config {
        let p = convert_gpio_str_to_output_pin(chip, c.pin.clone(), c.low_active);
        let low_active = c.low_active;
        let processor_init = quote! {
            let mut #processor_ident = ::rmk::builtin_processor::led_indicator::KeyboardIndicatorProcessor::new(
                #p,
                #low_active,
                ::rmk::types::led_indicator::LedIndicatorType::#led_indicator_variant,
            );
        };
        initializers.extend(processor_init);
        executors.push(quote! { #processor_ident.run() });
    }
}

fn expand_custom_processor(fn_item: &syn::ItemFn) -> (TokenStream, &syn::Ident) {
    let task_name = &fn_item.sig.ident;

    let content = &fn_item.block.stmts;
    let initializer = quote! {
        let mut #task_name = {
            #(#content)*
        };
    };

    (initializer, task_name)
}
