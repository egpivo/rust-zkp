use zkp::commitment::commit;
use num_bigint::BigUint;

fn main() {
    let p = BigUint::from(23u32);
    let g = BigUint::from(4u32);
    let h = BigUint::from(9u32);

    println!( "{}",
        commit(&BigUint::from(333u32), &BigUint::from(3u32), &g, &h, &p)
    )
}