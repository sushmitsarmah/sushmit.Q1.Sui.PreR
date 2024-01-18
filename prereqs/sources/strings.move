module prereqs::strings {
    use sui::object::{Self, UID};
    use sui::tx_context::TxContext;

    use std::string::{Self, String};

    struct Name has key, store {
        id: UID,
        name: String
    }

    public fun issue_name_nft (
        name_bytes: vector<u8>, ctx: &mut TxContext
    ): Name {
        Name {
            id: object::new(ctx),
            name: string::utf8(name_bytes)
        }
    }
}