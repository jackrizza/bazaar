use diesel::prelude::*;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::patrons)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Patron {
    pub id: i32,
    pub peer_id: String,
    pub public_key: String,
}

impl Patron {
    pub fn create_patron(
        peer_id: String,
        public_key: String,
        conn: &mut SqliteConnection,
    ) -> Result<Patron, diesel::result::Error> {
        use crate::schema::patrons;

        #[derive(Insertable)]
        #[table_name = "patrons"]
        struct NewPatron {
            peer_id: String,
            public_key: String,
        }

        let new_patron = NewPatron {
            peer_id,
            public_key,
        };

        diesel::insert_into(patrons::table)
            .values(&new_patron)
            .execute(conn)?;

        patrons::table.order(patrons::id.desc()).first(conn)
    }

    pub fn get_patron_from_peer_id(
        pid: String,
        conn: &mut SqliteConnection,
    ) -> Result<Patron, diesel::result::Error> {
        use crate::schema::patrons::dsl::*;

        patrons.filter(peer_id.eq(pid)).first::<Patron>(conn)
    }

    pub fn remove_from_peer_id(
        pid: String,
        conn: &mut SqliteConnection,
    ) -> Result<usize, diesel::result::Error> {
        use crate::schema::patrons::dsl::*;

        diesel::delete(patrons.filter(peer_id.eq(pid))).execute(conn)
    }

    pub fn get_peer_id_from_public_key(
        pkey: String,
        conn: &mut SqliteConnection,
    ) -> Result<String, diesel::result::Error> {
        use crate::schema::patrons::dsl::*;

        patrons
            .filter(public_key.eq(pkey))
            .select(peer_id)
            .first::<String>(conn)
    }
}
