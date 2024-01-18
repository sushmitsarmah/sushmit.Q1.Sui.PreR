module prereqs::Entry {
    use sui::transfer;
    use sui::object::{Self, UID};
    use sui::tx_context::TxContext;

    struct Object has key {
        id: UID
    }

    public fun create(ctx: &mut TxContext): Object {
        Object { id: object::new(ctx) }
    }

    entry fun create_and_transfer(to: address, ctx: &mut TxContext) {
        transfer::transfer(create(ctx), to)
    }
}