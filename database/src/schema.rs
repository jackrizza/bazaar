// @generated automatically by Diesel CLI.

diesel::table! {
    patrons (id) {
        id -> Integer,
        peer_id -> Text,
        public_key -> Text,
    }
}
