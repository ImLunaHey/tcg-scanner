use crate::{Database, decode, storage};
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRecord,
};
impl Database {
    /// A single SQLite cursor gives a consistent newest-first inventory snapshot.
    /// The visitor must not re-enter this database or perform external I/O.
    pub fn read_inventory_records(
        &self,
        mut visit: impl FnMut(ScanRecord) -> Result<()>,
    ) -> Result<()> {
        self.read(|db| {
            let mut query = db.prepare("SELECT record FROM jobs WHERE state IN ('completed','failed','cancelled') ORDER BY created DESC,id").map_err(storage)?;
            let mut rows=query.query([]).map_err(storage)?;
            let mut count=0;
            while let Some(row)=rows.next().map_err(storage)? {
                count+=1;
                if count>100_000 {
                    return Err(ScannerError::new("resourceLimit","Inventory exceeds 100,000 saved jobs."));
                }
                visit(decode(row.get(0).map_err(storage)?)?)?;
            }
            Ok(())
        })
    }
}
