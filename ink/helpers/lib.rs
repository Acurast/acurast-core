#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub type OuterError<T> = Result<Result<T, ink::LangError>, ink::env::Error>;
