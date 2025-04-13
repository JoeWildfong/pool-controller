pico-target := "thumbv6m-none-eabi"

cargo CMD *ARGS:
    cargo {{CMD}} --target={{pico-target}} --features device {{ARGS}}

uf2 PROFILE *ARGS:
    elf2uf2-rs target/thumbv6m-none-eabi/{{PROFILE}}/pool_controller target/pool_controller_{{PROFILE}}.uf2
