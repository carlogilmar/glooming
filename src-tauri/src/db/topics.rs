//! The Hub's topics, and where each gloom sits in them.

use super::docs::now;
use super::models::{Placement, Topic};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

pub async fn list(pool: &SqlitePool, project_id: i64) -> AppResult<Vec<Topic>> {
    Ok(
        sqlx::query_as::<_, Topic>("SELECT * FROM topics WHERE project_id = ? ORDER BY position, id")
            .bind(project_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Topic> {
    sqlx::query_as::<_, Topic>("SELECT * FROM topics WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("topic {id}")))
}

/// A new topic goes last: it is the one you are about to fill, and putting it
/// anywhere else would reorder a reading path somebody may already be following.
pub async fn create(pool: &SqlitePool, project_id: i64, name: &str) -> AppResult<Topic> {
    let name = clean(name)?;
    let id = sqlx::query(
        "INSERT INTO topics (project_id, name, position, created_at)
         VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM topics WHERE project_id = ?), ?)",
    )
    .bind(project_id)
    .bind(&name)
    .bind(project_id)
    .bind(now())
    .execute(pool)
    .await?
    .last_insert_rowid();
    get(pool, id).await
}

pub async fn rename(pool: &SqlitePool, id: i64, name: &str) -> AppResult<Topic> {
    let name = clean(name)?;
    sqlx::query("UPDATE topics SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

/// Remove a topic. Its glooms move to Unsorted in the same transaction — a
/// topic is a heading, and taking a heading away must never take the notes
/// under it.
pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE docs SET topic_id = NULL WHERE topic_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM topics WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Write the whole arrangement after a drag: the topics in order, and every
/// gloom's topic, its place being its index among the glooms listed for that
/// topic.
///
/// The whole thing rather than a move, because a drag changes several places at
/// once — the dragged row, and everything it slid past — and the Hub already
/// holds the result. One transaction, so a half-written order is never visible.
/// `updated_at` is deliberately left alone: filing a note is not writing it.
pub async fn arrange(
    pool: &SqlitePool,
    project_id: i64,
    topic_order: &[i64],
    glooms: &[Placement],
) -> AppResult<()> {
    let own: HashSet<i64> = list(pool, project_id).await?.iter().map(|t| t.id).collect();
    for id in topic_order.iter().chain(glooms.iter().filter_map(|g| g.topic_id.as_ref())) {
        if !own.contains(id) {
            return Err(AppError::BadInput(format!("topic {id} is not in this project")));
        }
    }

    let mut tx = pool.begin().await?;
    for (i, id) in topic_order.iter().enumerate() {
        sqlx::query("UPDATE topics SET position = ? WHERE id = ?")
            .bind(i as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let mut next: HashMap<Option<i64>, i64> = HashMap::new();
    for g in glooms {
        let at = next.entry(g.topic_id).or_insert(0);
        sqlx::query("UPDATE docs SET topic_id = ?, position = ? WHERE id = ? AND project_id = ?")
            .bind(g.topic_id)
            .bind(*at)
            .bind(g.id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        *at += 1;
    }
    tx.commit().await?;
    Ok(())
}

fn clean(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadInput("a topic needs a name".into()));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{docs, projects, test_pool};

    async fn project(pool: &SqlitePool) -> i64 {
        projects::touch(pool, "/code/app", "app").await.expect("project").id
    }

    #[tokio::test]
    async fn topics_are_created_last_and_listed_in_order() {
        let pool = test_pool().await;
        let p = project(&pool).await;
        create(&pool, p, "Start here").await.unwrap();
        create(&pool, p, "  Weather  ").await.unwrap();
        let names: Vec<_> = list(&pool, p).await.unwrap().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["Start here", "Weather"]);
    }

    #[tokio::test]
    async fn a_topic_needs_a_name() {
        let pool = test_pool().await;
        let p = project(&pool).await;
        assert!(create(&pool, p, "   ").await.is_err());
    }

    #[tokio::test]
    async fn removing_a_topic_keeps_its_glooms_in_unsorted() {
        let pool = test_pool().await;
        let p = project(&pool).await;
        let t = create(&pool, p, "Weather").await.unwrap();
        let d = docs::create(&pool, p, "Fan-out", "prose").await.unwrap();
        arrange(&pool, p, &[t.id], &[Placement { id: d.id, topic_id: Some(t.id) }]).await.unwrap();

        delete(&pool, t.id).await.unwrap();
        let back = docs::get(&pool, d.id).await.unwrap();
        assert_eq!(back.topic_id, None);
        assert_eq!(back.markdown, "prose");
    }

    #[tokio::test]
    async fn arranging_orders_topics_and_glooms_without_touching_updated_at() {
        let pool = test_pool().await;
        let p = project(&pool).await;
        let a = create(&pool, p, "A").await.unwrap();
        let b = create(&pool, p, "B").await.unwrap();
        let d1 = docs::create(&pool, p, "one", "").await.unwrap();
        let d2 = docs::create(&pool, p, "two", "").await.unwrap();
        let d3 = docs::create(&pool, p, "three", "").await.unwrap();

        arrange(
            &pool,
            p,
            &[b.id, a.id],
            &[
                Placement { id: d3.id, topic_id: Some(a.id) },
                Placement { id: d1.id, topic_id: Some(a.id) },
                Placement { id: d2.id, topic_id: None },
            ],
        )
        .await
        .unwrap();

        let names: Vec<_> = list(&pool, p).await.unwrap().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["B", "A"]);
        let one = docs::get(&pool, d1.id).await.unwrap();
        let three = docs::get(&pool, d3.id).await.unwrap();
        assert_eq!((three.topic_id, three.position), (Some(a.id), 0));
        assert_eq!((one.topic_id, one.position), (Some(a.id), 1));
        assert_eq!(one.updated_at, d1.updated_at, "filing a note is not writing it");
    }

    #[tokio::test]
    async fn another_projects_topic_is_refused() {
        let pool = test_pool().await;
        let p = project(&pool).await;
        let q = projects::touch(&pool, "/code/other", "other").await.unwrap().id;
        let theirs = create(&pool, q, "Theirs").await.unwrap();
        let d = docs::create(&pool, p, "mine", "").await.unwrap();
        let err = arrange(&pool, p, &[], &[Placement { id: d.id, topic_id: Some(theirs.id) }]).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn a_new_gloom_lands_at_the_top_of_unsorted() {
        let pool = test_pool().await;
        let p = project(&pool).await;
        let old = docs::create(&pool, p, "old", "").await.unwrap();
        let new = docs::create(&pool, p, "new", "").await.unwrap();
        assert!(new.position < old.position);
        let order: Vec<_> = docs::in_project(&pool, p).await.unwrap().into_iter().map(|d| d.title).collect();
        assert_eq!(order, ["new", "old"]);
    }
}
