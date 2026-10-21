#![cfg(feature = "serde")]
#[test]
fn test_serde_trait_implementations() {
    fn assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}

    assert_serde::<abacus::Value>();
    assert_serde::<abacus::Interval>();
    assert_serde::<abacus::Hash>();
    assert_serde::<abacus::Date>();
    assert_serde::<abacus::EvalResult>();
    assert_serde::<abacus::Dimensions>();
    assert_serde::<abacus::Unit>();
}
