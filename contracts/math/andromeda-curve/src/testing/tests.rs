use super::mock::{
    error_initialization, proper_initialization, query_curve_config, query_plot_y_from_x, reset,
    update_curve_config,
};
use andromeda_math::curve::{CurveConfig, CurveType};
use andromeda_std::{amp::AndrAddr, error::ContractError};
use cosmwasm_std::{Decimal, StdError};
use test_case::test_case;

#[test]
fn test_instantiation() {
    proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        },
        None,
    );
}

#[test]
fn test_reset() {
    let (mut deps, _) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        },
        Some(vec![AndrAddr::from_string("user1")]),
    );

    let err_res = reset(deps.as_mut(), "user2").unwrap_err();
    assert_eq!(err_res, ContractError::Unauthorized {});

    reset(deps.as_mut(), "user1").unwrap();

    let err_res = query_curve_config(deps.as_ref()).unwrap_err();
    assert_eq!(err_res, ContractError::Std(StdError::NotFound { kind: "type: andromeda_math::curve::CurveConfig; key: [63, 75, 72, 76, 65, 5F, 63, 6F, 6E, 66, 69, 67]".to_string() }));

    update_curve_config(
        deps.as_mut(),
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: Some(2),
        },
        "user1",
    )
    .unwrap();

    let res = query_curve_config(deps.as_ref()).unwrap().curve_config;
    assert_eq!(
        res,
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: Some(2),
        }
    );
}

#[test]
fn test_update_curve_config() {
    let (mut deps, _) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        },
        Some(vec![AndrAddr::from_string("user1")]),
    );
    let err_res = update_curve_config(
        deps.as_mut(),
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: Some(2),
        },
        "user2",
    )
    .unwrap_err();
    assert_eq!(err_res, ContractError::Unauthorized {});

    update_curve_config(
        deps.as_mut(),
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: Some(2),
        },
        "user1",
    )
    .unwrap();

    let res = query_curve_config(deps.as_ref()).unwrap().curve_config;
    assert_eq!(
        res,
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: Some(2),
        }
    );
}

#[test]
fn test_query_curve_config() {
    let (deps, _info) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        },
        None,
    );
    let res = query_curve_config(deps.as_ref()).unwrap().curve_config;
    assert_eq!(
        res,
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        }
    );
}

#[test]
fn test_query_curve_config_base_is_0() {
    let err_res = error_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 0,
            multiple_variable_value: None,
            constant_value: None,
        },
        None,
    );
    assert_eq!(
        err_res,
        ContractError::CustomError {
            msg: "Base Value must be bigger than Zero".to_string()
        }
    );
}

#[test_case(2, "4".to_string() ; "exp(2, 2)")]
#[test_case(3, "8".to_string() ; "exp(2, 3)")]
#[test_case(4, "16".to_string() ; "exp(2, 4)")]
fn test_query_plot_y_from_x_base_2_growth(input_x: u64, expected_y: String) {
    let input_x = Decimal::from_atomics(input_x, 18).unwrap();
    let (deps, _info) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        },
        None,
    );

    let res = query_plot_y_from_x(deps.as_ref(), input_x).unwrap().y_value;
    assert_eq!(res.to_string(), expected_y);
}

#[test_case(2, "9".to_string() ; "exp(3, 2)")]
#[test_case(3, "27".to_string() ; "exp(3, 3)")]
#[test_case(4, "81".to_string() ; "exp(3, 4)")]
fn test_query_plot_y_from_x_base_3_growth(input_x: u64, expected_y: String) {
    let input_x = Decimal::from_atomics(input_x, 18).unwrap();
    let (deps, _info) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 3,
            multiple_variable_value: None,
            constant_value: None,
        },
        None,
    );

    let res = query_plot_y_from_x(deps.as_ref(), input_x).unwrap().y_value;
    assert_eq!(res.to_string(), expected_y);
}

#[test_case(2, "32".to_string() ; "exp(4, 2)")]
#[test_case(3, "128".to_string() ; "exp(4, 3)")]
#[test_case(4, "512".to_string() ; "exp(4, 4)")]
fn test_query_plot_y_from_x_base_4_growth_constant_2(input_x: u64, expected_y: String) {
    let input_x = Decimal::from_atomics(input_x, 18).unwrap();
    let (deps, _info) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Growth,
            base_value: 4,
            multiple_variable_value: None,
            constant_value: Some(2),
        },
        None,
    );

    let res = query_plot_y_from_x(deps.as_ref(), input_x).unwrap().y_value;
    assert_eq!(res.to_string(), expected_y);
}

#[test_case(2, "0.25".to_string() ; "exp(1/2, 2)")]
#[test_case(3, "0.125".to_string() ; "exp(1/2, 3)")]
#[test_case(4, "0.0625".to_string() ; "exp(1/2, 4)")]
fn test_query_plot_y_from_x_base_2_decay(input_x: u64, expected_y: String) {
    let input_x = Decimal::from_atomics(input_x, 18).unwrap();
    let (deps, _info) = proper_initialization(
        CurveConfig::ExpConfig {
            curve_type: CurveType::Decay,
            base_value: 2,
            multiple_variable_value: None,
            constant_value: None,
        },
        None,
    );

    let res = query_plot_y_from_x(deps.as_ref(), input_x).unwrap().y_value;
    assert_eq!(res.to_string(), expected_y);
}
