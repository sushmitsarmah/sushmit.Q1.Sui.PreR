module prereqs::one_timer_init {
    use sui::transfer;
    use sui::object::{Self, UID};
    use sui::tx_context::{Self, TxContext};

    struct CreatorCapability has key {
        id: UID
    }

    // only module author will own a version of a
    /// `CreatorCapability` struct.
    fun init(ctx: &mut TxContext) {
        transfer::transfer(CreatorCapability {
            id: object::new(ctx),
        }, tx_context::sender(ctx))
    }
}