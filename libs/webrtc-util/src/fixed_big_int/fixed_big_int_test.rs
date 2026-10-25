use super::*;

#[test]
fn test_fixed_big_int_set_bit() {
    let mut bi = FixedBigInt::new(224);

    bi.set_bit(0);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000000000000000000000000001"
    );

    bi.lsh(1);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000000000000000000000000002"
    );

    bi.lsh(0);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000000000000000000000000002"
    );

    bi.set_bit(10);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000000000000000000000000402"
    );
    bi.lsh(20);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000000000000000000040200000"
    );

    bi.set_bit(80);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000000100000000000040200000"
    );
    bi.lsh(4);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000000000000001000000000000402000000"
    );

    bi.set_bit(130);
    assert_eq!(
        bi.to_string(),
        "0000000000000000000000000000000400000000001000000000000402000000"
    );
    bi.lsh(64);
    assert_eq!(
        bi.to_string(),
        "0000000000000004000000000010000000000004020000000000000000000000"
    );

    bi.set_bit(7);
    assert_eq!(
        bi.to_string(),
        "0000000000000004000000000010000000000004020000000000000000000080"
    );

    bi.lsh(129);
    assert_eq!(
        bi.to_string(),
        "0000000004000000000000000000010000000000000000000000000000000000"
    );

    for _ in 0..256 {
        bi.lsh(1);
        bi.set_bit(0);
    }
    assert_eq!(
        bi.to_string(),
        "00000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
    );
}
