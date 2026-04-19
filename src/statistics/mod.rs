//! Classical statistics (descriptive moments, order statistics, association, shape, OLS).
//!
//! This crate module is separate from [`crate::indicators`] (market / TA studies). Use it for
//! distribution summaries, correlations, and simple linear models on numeric slices.

#![allow(clippy::pedantic, clippy::nursery)] // Numeric stats helpers; pedantic naming noise vs math clarity.

pub mod association;
pub mod descriptive;
pub mod rank;
pub mod regression;
pub mod shape;

pub use association::{pearson_correlation, sample_covariance, spearman_correlation};
pub use descriptive::{
    mean, population_std, population_variance, sample_std, sample_variance, standard_error_mean,
    weighted_mean,
};
pub use rank::{interquartile_range, median, percentile_linear, quartiles, rank_average};
pub use regression::{OlsFit, ols_simple};
pub use shape::{sample_excess_kurtosis, sample_skewness};
