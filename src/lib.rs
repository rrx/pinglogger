pub mod icmp;
pub mod pinger;
pub mod cli;
pub mod stats;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
