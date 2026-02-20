// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Query builder for database operations in skylet-plugin-common v0.3.0
use crate::database::{DatabaseError, DatabaseValue, ToSql};
use std::collections::HashMap;

/// Order direction for query sorting
#[derive(Debug, Clone, PartialEq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

impl OrderDirection {
    pub fn as_sql(&self) -> &'static str {
        match self {
            OrderDirection::Asc => "ASC",
            OrderDirection::Desc => "DESC",
        }
    }
}

/// WHERE clause condition
#[derive(Debug, Clone)]
pub enum WhereClause {
    /// Simple equality condition
    Equals {
        column: String,
        value: String, // Simplified - in real implementation would use trait objects
    },
    /// Simple inequality condition
    NotEquals {
        column: String,
        value: String, // Simplified
    },
    /// Greater than condition
    GreaterThan {
        column: String,
        value: String, // Simplified
    },
    /// Greater than or equal condition
    GreaterThanOrEqual {
        column: String,
        value: String, // Simplified
    },
    /// Less than condition
    LessThan {
        column: String,
        value: String, // Simplified
    },
    /// Less than or equal condition
    LessThanOrEqual {
        column: String,
        value: String, // Simplified
    },
    /// LIKE condition
    Like { column: String, pattern: String },
    /// IN condition
    In {
        column: String,
        values: Vec<String>, // Simplified
    },
    /// NOT IN condition
    NotIn {
        column: String,
        values: Vec<String>, // Simplified
    },
    /// IS NULL condition
    IsNull { column: String },
    /// IS NOT NULL condition
    IsNotNull { column: String },
    /// AND condition - combines multiple clauses
    And(Vec<WhereClause>),
    /// OR condition - combines multiple clauses
    Or(Vec<WhereClause>),
    /// Raw SQL condition
    Raw {
        sql: String,
        params: Vec<String>, // Simplified
    },
}

impl WhereClause {
    /// Create an equals condition
    pub fn eq(column: &str, value: String) -> Self {
        Self::Equals {
            column: column.to_string(),
            value,
        }
    }

    /// Create a not equals condition
    pub fn ne(column: &str, value: String) -> Self {
        Self::NotEquals {
            column: column.to_string(),
            value,
        }
    }

    /// Create a greater than condition
    pub fn gt(column: &str, value: String) -> Self {
        Self::GreaterThan {
            column: column.to_string(),
            value,
        }
    }

    /// Create a greater than or equal condition
    pub fn gte(column: &str, value: String) -> Self {
        Self::GreaterThanOrEqual {
            column: column.to_string(),
            value,
        }
    }

    /// Create a less than condition
    pub fn lt(column: &str, value: String) -> Self {
        Self::LessThan {
            column: column.to_string(),
            value,
        }
    }

    /// Create a less than or equal condition
    pub fn lte(column: &str, value: String) -> Self {
        Self::LessThanOrEqual {
            column: column.to_string(),
            value,
        }
    }

    /// Create a LIKE condition
    pub fn like(column: &str, pattern: &str) -> Self {
        Self::Like {
            column: column.to_string(),
            pattern: pattern.to_string(),
        }
    }

    /// Create an IN condition
    pub fn in_list(column: &str, values: Vec<String>) -> Self {
        Self::In {
            column: column.to_string(),
            values,
        }
    }

    /// Create a NOT IN condition
    pub fn not_in_list(column: &str, values: Vec<String>) -> Self {
        Self::NotIn {
            column: column.to_string(),
            values,
        }
    }

    /// Create an IS NULL condition
    pub fn is_null(column: &str) -> Self {
        Self::IsNull {
            column: column.to_string(),
        }
    }

    /// Create an IS NOT NULL condition
    pub fn is_not_null(column: &str) -> Self {
        Self::IsNotNull {
            column: column.to_string(),
        }
    }

    /// Create an AND condition
    pub fn and(conditions: Vec<WhereClause>) -> Self {
        Self::And(conditions)
    }

    /// Create an OR condition
    pub fn or(conditions: Vec<WhereClause>) -> Self {
        Self::Or(conditions)
    }

    /// Create a raw SQL condition
    pub fn raw(sql: &str, params: Vec<String>) -> Self {
        Self::Raw {
            sql: sql.to_string(),
            params,
        }
    }
}

/// ORDER BY clause
#[derive(Debug, Clone)]
pub struct OrderBy {
    pub column: String,
    pub direction: OrderDirection,
}

impl OrderBy {
    pub fn asc(column: &str) -> Self {
        Self {
            column: column.to_string(),
            direction: OrderDirection::Asc,
        }
    }

    pub fn desc(column: &str) -> Self {
        Self {
            column: column.to_string(),
            direction: OrderDirection::Desc,
        }
    }

    pub fn new(column: &str, direction: OrderDirection) -> Self {
        Self {
            column: column.to_string(),
            direction,
        }
    }
}

/// JOIN clause
#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: String,
    pub on_clause: WhereClause,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

impl JoinType {
    pub fn as_sql(&self) -> &'static str {
        match self {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL JOIN",
            JoinType::Cross => "CROSS JOIN",
        }
    }
}

/// Query builder for constructing SQL queries
pub struct QueryBuilder {
    table: String,
    alias: Option<String>,
    selects: Vec<String>,
    joins: Vec<JoinClause>,
    wheres: Vec<WhereClause>,
    order_by: Vec<OrderBy>,
    limit: Option<usize>,
    offset: Option<usize>,
    group_by: Vec<String>,
    having: Vec<WhereClause>,
    parameters: Vec<Box<dyn ToSql + Send + Sync>>,
    distinct: bool,
}

impl QueryBuilder {
    /// Create a new SELECT query builder
    pub fn select(table: &str) -> Self {
        Self {
            table: table.to_string(),
            alias: None,
            selects: vec!["*".to_string()],
            joins: Vec::new(),
            wheres: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            group_by: Vec::new(),
            having: Vec::new(),
            parameters: Vec::new(),
            distinct: false,
        }
    }

    /// Create a new SELECT query with specific columns
    pub fn select_columns(table: &str, columns: &[&str]) -> Self {
        Self {
            table: table.to_string(),
            alias: None,
            selects: columns.iter().map(|c| c.to_string()).collect(),
            joins: Vec::new(),
            wheres: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            group_by: Vec::new(),
            having: Vec::new(),
            parameters: Vec::new(),
            distinct: false,
        }
    }

    /// Set table alias
    pub fn alias(mut self, alias: &str) -> Self {
        self.alias = Some(alias.to_string());
        self
    }

    /// Set DISTINCT flag
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Add columns to SELECT
    pub fn columns(mut self, columns: &[&str]) -> Self {
        if self.selects.len() == 1 && self.selects[0] == "*" {
            self.selects.clear();
        }
        self.selects.extend(columns.iter().map(|c| c.to_string()));
        self
    }

    /// Add a column to SELECT
    pub fn column(mut self, column: &str) -> Self {
        self.selects.push(column.to_string());
        self
    }

    /// Add a WHERE clause
    pub fn where_clause(mut self, condition: WhereClause) -> Self {
        self.wheres.push(condition);
        self
    }

    /// Add WHERE clause (convenience method)
    pub fn where_eq(self, column: &str, value: String) -> Self {
        self.where_clause(WhereClause::eq(column, value))
    }

    /// Add WHERE clause for multiple conditions with AND
    pub fn where_and(mut self, conditions: Vec<WhereClause>) -> Self {
        if conditions.len() == 1 {
            self.wheres.push(conditions.into_iter().next().unwrap());
        } else {
            self.wheres.push(WhereClause::and(conditions));
        }
        self
    }

    /// Add a JOIN clause
    pub fn join(mut self, join: JoinClause) -> Self {
        self.joins.push(join);
        self
    }

    /// Add INNER JOIN
    pub fn inner_join(self, table: &str, on: WhereClause) -> Self {
        self.join(JoinClause {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on_clause: on,
            alias: None,
        })
    }

    /// Add INNER JOIN with alias
    pub fn inner_join_alias(self, table: &str, alias: &str, on: WhereClause) -> Self {
        self.join(JoinClause {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on_clause: on,
            alias: Some(alias.to_string()),
        })
    }

    /// Add LEFT JOIN
    pub fn left_join(self, table: &str, on: WhereClause) -> Self {
        self.join(JoinClause {
            join_type: JoinType::Left,
            table: table.to_string(),
            on_clause: on,
            alias: None,
        })
    }

    /// Add ORDER BY
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by.push(order);
        self
    }

    /// Add ORDER BY ASC
    pub fn order_by_asc(self, column: &str) -> Self {
        self.order_by(OrderBy::asc(column))
    }

    /// Add ORDER BY DESC
    pub fn order_by_desc(self, column: &str) -> Self {
        self.order_by(OrderBy::desc(column))
    }

    /// Set LIMIT
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set OFFSET
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Add GROUP BY
    pub fn group_by(mut self, columns: &[&str]) -> Self {
        self.group_by.extend(columns.iter().map(|c| c.to_string()));
        self
    }

    /// Add HAVING clause
    pub fn having(mut self, condition: WhereClause) -> Self {
        self.having.push(condition);
        self
    }

    /// Build the query and return SQL and parameters
    pub fn build(self) -> Result<(String, Vec<Box<dyn ToSql + Send + Sync>>), DatabaseError> {
        let mut sql_parts = Vec::new();

        // SELECT clause
        let distinct_clause = if self.distinct { "DISTINCT " } else { "" };
        let select_clause = format!("SELECT {}{}", distinct_clause, self.selects.join(", "));
        sql_parts.push(select_clause);

        // FROM clause
        let from_clause = if let Some(ref alias) = self.alias {
            format!("FROM {} AS {}", self.table, alias)
        } else {
            format!("FROM {}", self.table)
        };
        sql_parts.push(from_clause);

        // JOIN clauses
        for join in &self.joins {
            let join_clause = if let Some(ref alias) = join.alias {
                format!(
                    "{} {} AS {} ON {}",
                    join.join_type.as_sql(),
                    join.table,
                    alias,
                    self.build_where_clause(&join.on_clause)?
                )
            } else {
                format!(
                    "{} {} ON {}",
                    join.join_type.as_sql(),
                    join.table,
                    self.build_where_clause(&join.on_clause)?
                )
            };
            sql_parts.push(join_clause);
        }

        // WHERE clauses
        if !self.wheres.is_empty() {
            let where_sql = if self.wheres.len() == 1 {
                format!("WHERE {}", self.build_where_clause(&self.wheres[0])?)
            } else {
                let conditions: Result<Vec<_>, DatabaseError> = self
                    .wheres
                    .iter()
                    .map(|w| self.build_where_clause(w))
                    .collect();
                format!("WHERE {}", conditions?.join(" AND "))
            };
            sql_parts.push(where_sql);
        }

        // GROUP BY clauses
        if !self.group_by.is_empty() {
            sql_parts.push(format!("GROUP BY {}", self.group_by.join(", ")));
        }

        // HAVING clauses
        if !self.having.is_empty() {
            let having_sql = if self.having.len() == 1 {
                format!("HAVING {}", self.build_where_clause(&self.having[0])?)
            } else {
                let conditions: Result<Vec<_>, DatabaseError> = self
                    .having
                    .iter()
                    .map(|h| self.build_where_clause(h))
                    .collect();
                format!("HAVING {}", conditions?.join(" AND "))
            };
            sql_parts.push(having_sql);
        }

        // ORDER BY clauses
        if !self.order_by.is_empty() {
            let order_clauses: Vec<_> = self
                .order_by
                .iter()
                .map(|o| format!("{} {}", o.column, o.direction.as_sql()))
                .collect();
            sql_parts.push(format!("ORDER BY {}", order_clauses.join(", ")));
        }

        // LIMIT clause
        if let Some(limit) = self.limit {
            sql_parts.push(format!("LIMIT {}", limit));
        }

        // OFFSET clause
        if let Some(offset) = self.offset {
            sql_parts.push(format!("OFFSET {}", offset));
        }

        let sql = sql_parts.join(" ");
        Ok((sql, self.parameters))
    }

    /// Build a WHERE clause recursively
    fn build_where_clause(&self, clause: &WhereClause) -> Result<String, DatabaseError> {
        match clause {
            WhereClause::Equals { column, .. } => Ok(format!("{} = {}", column, "?")),
            WhereClause::NotEquals { column, .. } => Ok(format!("{} != {}", column, "?")),
            WhereClause::GreaterThan { column, .. } => Ok(format!("{} > {}", column, "?")),
            WhereClause::GreaterThanOrEqual { column, .. } => Ok(format!("{} >= {}", column, "?")),
            WhereClause::LessThan { column, .. } => Ok(format!("{} < {}", column, "?")),
            WhereClause::LessThanOrEqual { column, .. } => Ok(format!("{} <= {}", column, "?")),
            WhereClause::Like { column, pattern } => Ok(format!("{} LIKE '{}'", column, pattern)),
            WhereClause::In { column, values } => {
                let placeholders: Vec<_> = values.iter().map(|_| "?".to_string()).collect();
                Ok(format!("{} IN ({})", column, placeholders.join(", ")))
            }
            WhereClause::NotIn { column, values } => {
                let placeholders: Vec<_> = values.iter().map(|_| "?".to_string()).collect();
                Ok(format!("{} NOT IN ({})", column, placeholders.join(", ")))
            }
            WhereClause::IsNull { column } => Ok(format!("{} IS NULL", column)),
            WhereClause::IsNotNull { column } => Ok(format!("{} IS NOT NULL", column)),
            WhereClause::And(conditions) => {
                let clauses: Result<Vec<_>, DatabaseError> = conditions
                    .iter()
                    .map(|c| self.build_where_clause(c))
                    .collect();
                Ok(format!("({})", clauses?.join(" AND ")))
            }
            WhereClause::Or(conditions) => {
                let clauses: Result<Vec<_>, DatabaseError> = conditions
                    .iter()
                    .map(|c| self.build_where_clause(c))
                    .collect();
                Ok(format!("({})", clauses?.join(" OR ")))
            }
            WhereClause::Raw { sql, .. } => Ok(sql.clone()),
        }
    }
}

/// INSERT query builder
pub struct InsertBuilder {
    table: String,
    columns: Vec<String>,
    values: Vec<Vec<Box<dyn ToSql + Send + Sync>>>,
    on_conflict: Option<String>,
    returning: Vec<String>,
}

impl InsertBuilder {
    /// Create a new INSERT builder
    pub fn into(table: &str) -> Self {
        Self {
            table: table.to_string(),
            columns: Vec::new(),
            values: Vec::new(),
            on_conflict: None,
            returning: Vec::new(),
        }
    }

    /// Set columns to insert
    pub fn columns(mut self, columns: &[&str]) -> Self {
        self.columns = columns.iter().map(|c| c.to_string()).collect();
        self
    }

    /// Add a row of values
    pub fn values<T: ToSql + Send + Sync + 'static>(mut self, row: Vec<T>) -> Self {
        let converted: Vec<Box<dyn ToSql + Send + Sync>> = row
            .into_iter()
            .map(|v| Box::new(v) as Box<dyn ToSql + Send + Sync>)
            .collect();
        self.values.push(converted);
        self
    }

    /// Add ON CONFLICT clause (PostgreSQL specific)
    pub fn on_conflict(mut self, clause: &str) -> Self {
        self.on_conflict = Some(clause.to_string());
        self
    }

    /// Add RETURNING clause
    pub fn returning(mut self, columns: &[&str]) -> Self {
        self.returning = columns.iter().map(|c| c.to_string()).collect();
        self
    }

    /// Build the INSERT query
    pub fn build(self) -> Result<(String, Vec<Box<dyn ToSql + Send + Sync>>), DatabaseError> {
        if self.columns.is_empty() || self.values.is_empty() {
            return Err(DatabaseError::query("INSERT requires columns and values"));
        }

        let mut sql_parts = Vec::new();
        let mut all_parameters = Vec::new();

        // INSERT INTO table (columns)
        sql_parts.push(format!(
            "INSERT INTO {} ({})",
            self.table,
            self.columns.join(", ")
        ));

        // VALUES clause
        let values_clauses: Result<Vec<_>, DatabaseError> = self
            .values
            .into_iter()
            .map(|row| {
                all_parameters.extend(row);
                let placeholders: Vec<_> =
                    (0..self.columns.len()).map(|_| "?".to_string()).collect();
                Ok(format!("({})", placeholders.join(", ")))
            })
            .collect();

        sql_parts.push(format!("VALUES {}", values_clauses?.join(", ")));

        // ON CONFLICT clause
        if let Some(clause) = self.on_conflict {
            sql_parts.push(format!("ON CONFLICT {}", clause));
        }

        // RETURNING clause
        if !self.returning.is_empty() {
            sql_parts.push(format!("RETURNING {}", self.returning.join(", ")));
        }

        let sql = sql_parts.join(" ");
        Ok((sql, all_parameters))
    }
}

/// UPDATE query builder
pub struct UpdateBuilder {
    table: String,
    sets: Vec<(String, Box<dyn ToSql + Send + Sync>)>,
    wheres: Vec<WhereClause>,
    returning: Vec<String>,
}

impl UpdateBuilder {
    /// Create a new UPDATE builder
    pub fn update(table: &str) -> Self {
        Self {
            table: table.to_string(),
            sets: Vec::new(),
            wheres: Vec::new(),
            returning: Vec::new(),
        }
    }

    /// Add SET clause
    pub fn set<T: ToSql + Send + Sync + 'static>(mut self, column: &str, value: T) -> Self {
        self.sets.push((
            column.to_string(),
            Box::new(value) as Box<dyn ToSql + Send + Sync>,
        ));
        self
    }

    /// Add WHERE clause
    pub fn where_clause(mut self, condition: WhereClause) -> Self {
        self.wheres.push(condition);
        self
    }

    /// Add RETURNING clause
    pub fn returning(mut self, columns: &[&str]) -> Self {
        self.returning = columns.iter().map(|c| c.to_string()).collect();
        self
    }

    /// Build the UPDATE query
    pub fn build(self) -> Result<(String, Vec<Box<dyn ToSql + Send + Sync>>), DatabaseError> {
        if self.sets.is_empty() {
            return Err(DatabaseError::query("UPDATE requires SET clauses"));
        }

        let mut sql_parts = Vec::new();
        let mut all_parameters = Vec::new();

        // UPDATE table
        sql_parts.push(format!("UPDATE {}", self.table));

        // SET clause
        let set_clauses: Vec<_> = self
            .sets
            .into_iter()
            .map(|(column, value)| {
                all_parameters.push(value);
                format!("{} = {}", column, "?")
            })
            .collect();
        sql_parts.push(format!("SET {}", set_clauses.join(", ")));

        // WHERE clauses
        if !self.wheres.is_empty() {
            let where_sql = if self.wheres.len() == 1 {
                // Build single where clause
                "?".to_string() // Placeholder - would need proper implementation
            } else {
                "?".to_string() // Placeholder - would need proper implementation
            };
            sql_parts.push(format!("WHERE {}", where_sql));
        }

        // RETURNING clause
        if !self.returning.is_empty() {
            sql_parts.push(format!("RETURNING {}", self.returning.join(", ")));
        }

        let sql = sql_parts.join(" ");
        Ok((sql, all_parameters))
    }
}

/// DELETE query builder
pub struct DeleteBuilder {
    table: String,
    wheres: Vec<WhereClause>,
    returning: Vec<String>,
}

impl DeleteBuilder {
    /// Create a new DELETE builder
    pub fn from(table: &str) -> Self {
        Self {
            table: table.to_string(),
            wheres: Vec::new(),
            returning: Vec::new(),
        }
    }

    /// Add WHERE clause
    pub fn where_clause(mut self, condition: WhereClause) -> Self {
        self.wheres.push(condition);
        self
    }

    /// Add RETURNING clause
    pub fn returning(mut self, columns: &[&str]) -> Self {
        self.returning = columns.iter().map(|c| c.to_string()).collect();
        self
    }

    /// Build the DELETE query
    pub fn build(self) -> Result<(String, Vec<Box<dyn ToSql + Send + Sync>>), DatabaseError> {
        let mut sql_parts = Vec::new();
        let all_parameters = Vec::new();

        // DELETE FROM table
        sql_parts.push(format!("DELETE FROM {}", self.table));

        // WHERE clauses
        if !self.wheres.is_empty() {
            let where_sql = if self.wheres.len() == 1 {
                "?".to_string() // Placeholder - would need proper implementation
            } else {
                "?".to_string() // Placeholder - would need proper implementation
            };
            sql_parts.push(format!("WHERE {}", where_sql));
        }

        // RETURNING clause
        if !self.returning.is_empty() {
            sql_parts.push(format!("RETURNING {}", self.returning.join(", ")));
        }

        let sql = sql_parts.join(" ");
        Ok((sql, all_parameters))
    }
}

/// Convenience functions for common query builders
pub fn select(table: &str) -> QueryBuilder {
    QueryBuilder::select(table)
}

pub fn select_columns(table: &str, columns: &[&str]) -> QueryBuilder {
    QueryBuilder::select_columns(table, columns)
}

pub fn insert_into(table: &str) -> InsertBuilder {
    InsertBuilder::into(table)
}

pub fn update(table: &str) -> UpdateBuilder {
    UpdateBuilder::update(table)
}

pub fn delete_from(table: &str) -> DeleteBuilder {
    DeleteBuilder::from(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let (sql, _) = select("users")
            .where_eq("id", "42".to_string())
            .order_by_asc("name")
            .limit(10)
            .build()
            .unwrap();

        assert!(sql.contains("SELECT * FROM users"));
        assert!(sql.contains("ORDER BY name ASC"));
        assert!(sql.contains("LIMIT 10"));
    }

    #[test]
    fn test_complex_select() {
        let (sql, _) = select("users")
            .columns(&["id", "name", "email"])
            .distinct()
            .left_join(
                "posts",
                WhereClause::eq("users.id", "posts.user_id".to_string()),
            )
            .where_eq("users.active", "true".to_string())
            .order_by_desc("users.created_at")
            .limit(20)
            .offset(10)
            .build()
            .unwrap();

        assert!(sql.contains("SELECT DISTINCT id, name, email FROM users"));
        assert!(sql.contains("LEFT JOIN posts ON users.id = ?"));
        assert!(sql.contains("ORDER BY users.created_at DESC"));
        assert!(sql.contains("LIMIT 20"));
        assert!(sql.contains("OFFSET 10"));
    }

    #[test]
    fn test_insert_builder() {
        let (sql, _) = insert_into("users")
            .columns(&["name", "email", "age"])
            .values(vec![
                "Alice".to_string(),
                "alice@example.com".to_string(),
                "25".to_string(),
            ])
            .values(vec![
                "Bob".to_string(),
                "bob@example.com".to_string(),
                "30".to_string(),
            ])
            .returning(&["id"])
            .build()
            .unwrap();

        assert!(sql.contains("INSERT INTO users (name, email, age)"));
        assert!(sql.contains("VALUES (?, ?, ?), (?, ?, ?)"));
        assert!(sql.contains("RETURNING id"));
    }

    #[test]
    fn test_where_clauses() {
        let eq_clause = WhereClause::eq("name", "Alice".to_string());
        let gt_clause = WhereClause::gt("age", "25".to_string());
        let in_clause =
            WhereClause::in_list("status", vec!["active".to_string(), "pending".to_string()]);
        let and_clause = WhereClause::and(vec![eq_clause, gt_clause]);
        let or_clause = WhereClause::or(vec![and_clause, in_clause]);

        let (sql, _): (String, Vec<Box<dyn ToSql + Send + Sync>>) =
            select("users").where_clause(or_clause).build().unwrap();

        assert!(sql.contains("WHERE"));
    }

    #[test]
    fn test_update_builder() {
        let (sql, _): (String, Vec<Box<dyn ToSql + Send + Sync>>) = update("users")
            .set("last_login", "2023-01-01")
            .where_clause(WhereClause::eq("id", "42".to_string()))
            .returning(&["updated_at"])
            .build()
            .unwrap();

        assert!(sql.contains("UPDATE users"));
        assert!(sql.contains("SET last_login = ?"));
        assert!(sql.contains("RETURNING updated_at"));
    }

    #[test]
    fn test_delete_builder() {
        let (sql, _): (String, Vec<Box<dyn ToSql + Send + Sync>>) = delete_from("users")
            .where_clause(WhereClause::lt("created_at", "2020-01-01".to_string()))
            .build()
            .unwrap();

        assert!(sql.contains("DELETE FROM users"));
        assert!(sql.contains("WHERE"));
    }
}
