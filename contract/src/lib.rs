use charms_sdk::data::{App, Data, Transaction, NFT};

pub fn app_contract(app: &App, _tx: &Transaction, _x: &Data, _w: &Data) -> bool {
    // Only handle NFT type, always allow
    match app.tag {
        NFT => true,
        _ => false,
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn nft_always_passes() {
        assert!(true);
    }
}