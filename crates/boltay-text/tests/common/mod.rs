/// One `#[test]` per case, so a failure names the exact phrase.
#[macro_export]
macro_rules! cases {
    ($config:expr; $($name:ident: $input:expr => $expected:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let config: boltay_text::Config = $config;
                assert_eq!(boltay_text::process(&config, $input), $expected, "input: {:?}", $input);
            }
        )*
    };
}

/// Cases where the text must come out exactly as it went in.
#[macro_export]
macro_rules! untouched {
    ($config:expr; $($name:ident: $input:expr,)*) => {
        $crate::cases! { $config; $($name: $input => $input,)* }
    };
}
