# Serde Traits and the `#[serde(with = ...)]` Override

What we just wrote in `serde_helpers.rs` uses several deep Rust trait concepts. This note explains them.

## The Code in Question

```rust
pub mod biguint_string {
    use num_bigint::BigUint;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &BigUint, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<BigUint, D::Error> {
        let s = String::deserialize(de)?;
        BigUint::parse_bytes(s.as_bytes(), 10)
            .ok_or_else(|| serde::de::Error::custom("invalid BigUint string"))
    }
}
```

Then in `Transaction`:

```rust
pub struct Transaction {
    // ...
    #[serde(with = "crate::serde_helpers::biguint_string")]
    pub challenge_e: BigUint,
}
```

What's happening here? Multiple trait concepts at once.

## Concept 1: Serializer / Deserializer Are Traits, Not Concrete Types

Serde is **format-agnostic**. The same struct can serialize to JSON, bincode, YAML, MessagePack — depending on which `Serializer` is plugged in.

```rust
fn serialize<S: Serializer>(v: &BigUint, ser: S) -> Result<S::Ok, S::Error> {
    //         ^^^^^^^^^^^^   generic over any Serializer
    ser.serialize_str(&v.to_string())
}
```

When called from JSON, `S` is `serde_json::Serializer` — `serialize_str` writes `"223"` to JSON.
When called from bincode, `S` is `bincode::Serializer` — `serialize_str` writes a length-prefixed UTF-8 string in binary.

We don't know or care which one — we just say "give me a `Serializer`".

This is **static dispatch with generics**: monomorphized at compile time, zero runtime overhead.

## Concept 2: Associated Types (`S::Ok`, `S::Error`)

Notice the return type:
```rust
Result<S::Ok, S::Error>
```

Trait `Serializer` has **associated types** for what it produces and what errors it raises:

```rust
trait Serializer {
    type Ok;
    type Error: Error;
    // ... methods that return Result<Self::Ok, Self::Error>
}
```

For `serde_json::Serializer`, `Ok = ()` and `Error = serde_json::Error`.
For `bincode::Serializer`, those are different types.

We write `S::Ok`, `S::Error` as opaque placeholders — "whatever this Serializer produces, that's our return type."

Associated types vs generics:
- `<S: Serializer>` — generic over Serializer
- `S::Ok` — the specific Ok-type *this* Serializer uses
- We don't add another generic for Ok because it's *fixed by the choice of S*

## Concept 3: Lifetimes (`'de`)

```rust
fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<BigUint, D::Error>
```

The `'de` lifetime means: "the deserializer borrows data with lifetime `'de`, and any `&str` we pull from it must live at least that long."

Why? Some serde formats deserialize **without copying** (zero-copy). For example, `&str` taken from a JSON buffer points *into* that buffer. The lifetime `'de` ties the deserialized data's lifetime to the input buffer's lifetime.

We're not using zero-copy here — we own the `String` we get back. But the lifetime parameter is still required because `Deserializer<'de>` is a generic trait parameterized by lifetime.

Reading shorthand:
- `'de` is just a name (could be `'a`, `'foo`)
- `D: Deserializer<'de>` says "D implements `Deserializer` with this borrow lifetime"
- `<'de, D: ...>` declares both, in that order

This is the most "Rust-flavored" piece. Don't try to memorize it; recognize the shape and move on.

## Concept 4: Trait Bounds and `?Sized`

We also use `String::deserialize(de)`. That's calling the `Deserialize` trait's method:

```rust
trait Deserialize<'de>: Sized {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error>;
}
```

`String` implements `Deserialize`, so `String::deserialize(de)` works. We then ask `BigUint::parse_bytes` to parse it.

The `Sized` bound on `Deserialize` says: "you must produce a value of known size at compile time" — which `String` and `BigUint` satisfy.

## Concept 5: The `#[serde(with = ...)]` Override

This is the killer feature.

By default, `#[derive(Serialize)]` on `Transaction` calls `BigUint::serialize` for `challenge_e`. `BigUint::serialize` (the default impl) writes it as `Vec<u32>` of internal limbs — `[223]` in JSON.

**`#[serde(with = "crate::serde_helpers::biguint_string")]`** overrides that: serde looks for two functions named `serialize` and `deserialize` in that module path, and uses **those** for this field instead of the default trait impl.

Effectively, a per-field codec replacement:

```rust
pub struct Transaction {
    pub from: u32,         // uses u32's default Serialize
    pub to: u32,           // uses u32's default Serialize
    #[serde(with = "...")]
    pub challenge_e: BigUint, // uses our custom functions instead
}
```

This is **not** trait override — `BigUint`'s `Serialize` impl still exists unchanged. It's a serde-derive-time decision: "for this field, route through this module's `serialize`/`deserialize` instead of calling the trait."

## Why It's Not Inheritance

Coming from OOP (Java, Python) you might think "BigUint is overriding its serialize method." That's **not** what's happening. Rust doesn't have method overriding in that sense.

What's happening:
- Serde's derive macro generates the `impl Serialize for Transaction { ... }` block
- For each field, it normally calls `field.serialize(serializer)`
- With `#[serde(with = "path")]`, it calls `path::serialize(&field, serializer)` instead
- The trait impl for `BigUint` is untouched; we're swapping out the **call site** in `Transaction`'s generated impl

Same `BigUint` type, two different places where it might be used:
- `Account.pubkey` (no override) — uses default limb-array serialization → bincode-friendly
- `Transaction.challenge_e` (override) — uses our string serialization → JSON-friendly

Same value, two formats. **Per-field codec choice**.

## Concept 6: Module Path as Codec Identifier

```rust
#[serde(with = "crate::serde_helpers::biguint_string")]
```

The string is a Rust path to a **module**. Serde's macro looks inside that module for two functions:
- `serialize<S: Serializer>(v: &T, ser: S) -> ...`
- `deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<T, D::Error>`

If you pointed at a function instead of a module, serde wouldn't find both halves.

The "module-as-codec" pattern is idiomatic in serde:
- `chrono` provides `chrono::serde::ts_seconds`, `chrono::serde::ts_milliseconds`, etc. for timestamps
- Each is a module containing the serialize/deserialize pair
- You pick which format you want per field

You can have many helpers:
```rust
pub mod biguint_string { ... }
pub mod biguint_hex { ... }     // could write 0xff format
pub mod biguint_decimal_padded { ... }
```

And a struct can mix them:
```rust
struct Foo {
    #[serde(with = "biguint_string")]
    a: BigUint,
    #[serde(with = "biguint_hex")]
    b: BigUint,
}
```

## Putting It All Together

```rust
pub fn serialize<S: Serializer>(v: &BigUint, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&v.to_string())
}
```

Reading this declaration:
- Generic over any `S` that implements `Serializer` (JSON, bincode, etc.)
- Takes a `&BigUint` and a `Serializer` instance
- Returns whatever the Serializer produces (or its error type)
- Calls `serialize_str` (a method on the trait) with the BigUint formatted as decimal

```rust
pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<BigUint, D::Error> {
    let s = String::deserialize(de)?;
    BigUint::parse_bytes(s.as_bytes(), 10)
        .ok_or_else(|| serde::de::Error::custom("invalid BigUint string"))
}
```

Reading:
- Generic over deserializer with lifetime `'de`
- First, ask the deserializer for a `String`
- Then parse it as decimal `BigUint`
- If parse fails, raise a serde error using the `Error::custom` constructor

## Why Rust's Trait System Makes This Possible

In dynamic languages, you'd swap behavior by monkey-patching or subclassing. In Rust:

| Concept | Rust mechanism |
|---------|----------------|
| Polymorphism over output format | Generic `S: Serializer` |
| Format-specific types | Associated types `S::Ok`, `S::Error` |
| Borrow vs own deserialized data | Lifetime `'de` |
| Per-field codec selection | `#[serde(with = "...")]` macro attribute |
| Compile-time dispatch | Monomorphization |

The result: zero-runtime-overhead format flexibility. Your generated code is as fast as if you'd hand-written every codec.

## Mental Model

When you see `#[serde(with = "path")]`:
> "For this field only, route serialization through `path::serialize` and deserialization through `path::deserialize` instead of using the field's default impl."

When you see `<S: Serializer>`:
> "Be polymorphic over format. The compiler will instantiate this function once per format used."

When you see `'de`:
> "Lifetime tying deserialized borrows to the input buffer."

When you see `S::Ok`:
> "The associated 'success type' of whatever Serializer was passed in."

That's enough to read 95% of serde-using Rust code.
