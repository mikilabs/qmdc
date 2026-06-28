use sqlparser::ast::*;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

/// Rewrite SQL query to add workspace filter
///
/// Adds `__workspace = ?` predicate to WHERE clause for all `FROM objects` and `JOIN objects`.
///
/// # Arguments
/// * `sql` - Original SQL query
/// * `workspace_id` - Workspace ID to filter by
///
/// # Returns
/// * `Ok(String)` - Rewritten SQL with workspace filter
/// * `Err(String)` - Error message if parsing fails
pub fn rewrite_sql_for_workspace(sql: &str, workspace_id: &str) -> Result<String, String> {
    // Parse SQL to AST
    let dialect = GenericDialect {};

    let ast = Parser::parse_sql(&dialect, sql).map_err(|e| format!("SQL parse error: {}", e))?;

    // Rewrite each statement
    let rewritten: Vec<Statement> = ast
        .into_iter()
        .map(|stmt| rewrite_statement(stmt, workspace_id))
        .collect();

    // Serialize back to SQL
    // Note: sqlparser doesn't have a built-in SQL formatter, so we'll need to manually construct SQL
    // For now, we'll use a simple approach: convert AST back to string
    Ok(format_statements(&rewritten))
}

fn rewrite_statement(stmt: Statement, workspace_id: &str) -> Statement {
    match stmt {
        Statement::Query(query) => Statement::Query(rewrite_query(*query, workspace_id)),
        other => other, // Other statement types (CREATE, INSERT, etc.) are not rewritten
    }
}

fn rewrite_query(query: Query, workspace_id: &str) -> Box<Query> {
    let mut new_query = query;

    // Rewrite CTEs (WITH clauses) if present
    if let Some(ref mut with) = new_query.with {
        for cte in &mut with.cte_tables {
            // Rewrite the query in each CTE
            let old_query = cte.query.clone();
            cte.query = rewrite_query(*old_query, workspace_id);
        }
    }

    // Rewrite the body (SELECT, UNION, etc.)
    *new_query.body = rewrite_set_expr(*new_query.body, workspace_id);

    Box::new(new_query)
}

fn rewrite_set_expr(expr: SetExpr, workspace_id: &str) -> SetExpr {
    match expr {
        SetExpr::Select(select) => SetExpr::Select(rewrite_select(*select, workspace_id)),
        SetExpr::Query(query) => {
            // Query variant wraps another query
            SetExpr::Query(rewrite_query(*query, workspace_id))
        }
        SetExpr::SetOperation {
            op,
            left,
            right,
            set_quantifier,
        } => {
            // Handle UNION, EXCEPT, INTERSECT
            SetExpr::SetOperation {
                op,
                left: Box::new(rewrite_set_expr(*left, workspace_id)),
                right: Box::new(rewrite_set_expr(*right, workspace_id)),
                set_quantifier,
            }
        }
        other => other,
    }
}

fn rewrite_select(mut select: Select, workspace_id: &str) -> Box<Select> {
    // First, recursively rewrite any subqueries in FROM/JOIN
    // This also adds RLS filters to JOIN ON clauses
    for table_with_joins in &mut select.from {
        rewrite_table_with_joins(table_with_joins, workspace_id);
    }

    // Rewrite subqueries in SELECT projections (columns)
    // NOTE: We do NOT modify function arguments to preserve DISTINCT
    for proj in &mut select.projection {
        rewrite_projection(proj, workspace_id);
    }

    // Rewrite subqueries in WHERE clause
    if let Some(ref mut where_expr) = select.selection {
        *where_expr = rewrite_expr(where_expr.clone(), workspace_id);
    }

    // Rewrite subqueries in GROUP BY
    if let GroupByExpr::Expressions(exprs) = &mut select.group_by {
        for expr in exprs {
            *expr = rewrite_expr(expr.clone(), workspace_id);
        }
    }

    // Add RLS filter to WHERE clause for main FROM table (if it's objects/edges)
    // Only for actual tables, not Derived (subqueries) - those are already filtered inside
    for table_with_joins in &select.from {
        // Only add filter for actual Table, not Derived (subqueries)
        if let TableFactor::Table { .. } = &table_with_joins.relation {
            if is_objects_table(&table_with_joins.relation) {
                // Check if there are any JOINs - if yes, we need alias
                let has_joins = !table_with_joins.joins.is_empty();

                let workspace_filter =
                    if let Some(alias) = get_table_alias(&table_with_joins.relation) {
                        // Use alias if available
                        create_workspace_filter_with_alias(alias, workspace_id)
                    } else if has_joins {
                        // If there are JOINs but no alias, use table name as alias
                        let table_name = get_table_name(&table_with_joins.relation)
                            .expect("Table should have name");
                        create_workspace_filter_with_alias(table_name, workspace_id)
                    } else {
                        // No alias and no JOINs - use unqualified __workspace
                        create_workspace_filter_unqualified(workspace_id)
                    };

                if let Some(existing_where) = &mut select.selection {
                    // If existing WHERE contains OR at top level, wrap it in parentheses
                    // to ensure correct operator precedence when adding AND workspace_filter
                    // Without parentheses: "A OR B AND C" = "A OR (B AND C)" (wrong!)
                    // With parentheses: "(A OR B) AND C" (correct!)
                    let left_expr = if matches!(
                        existing_where,
                        Expr::BinaryOp {
                            op: BinaryOperator::Or,
                            ..
                        }
                    ) {
                        Expr::Nested(Box::new(existing_where.clone()))
                    } else {
                        existing_where.clone()
                    };

                    let and_expr = Expr::BinaryOp {
                        left: Box::new(left_expr),
                        op: BinaryOperator::And,
                        right: Box::new(workspace_filter),
                    };
                    select.selection = Some(and_expr);
                } else {
                    select.selection = Some(workspace_filter);
                }
                break; // Only add once for the main table
            }
        }
    }

    Box::new(select)
}

fn rewrite_projection(proj: &mut SelectItem, workspace_id: &str) {
    match proj {
        SelectItem::UnnamedExpr(expr) => {
            *expr = rewrite_expr(expr.clone(), workspace_id);
        }
        SelectItem::ExprWithAlias { expr, .. } => {
            *expr = rewrite_expr(expr.clone(), workspace_id);
        }
        _ => {} // Wildcard doesn't need rewriting
    }
}

fn rewrite_expr(expr: Expr, workspace_id: &str) -> Expr {
    match expr {
        Expr::Subquery(query) => {
            // Recursively rewrite the subquery
            Expr::Subquery(rewrite_query(*query, workspace_id))
        }
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(rewrite_expr(*left, workspace_id)),
            op,
            right: Box::new(rewrite_expr(*right, workspace_id)),
        },
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op,
            expr: Box::new(rewrite_expr(*expr, workspace_id)),
        },
        Expr::Function(func) => {
            // CRITICAL: Do NOT modify function arguments to preserve DISTINCT
            // Only rewrite subqueries in function arguments, but preserve function structure
            let mut new_func = func.clone();
            for arg in &mut new_func.args {
                match arg {
                    FunctionArg::Unnamed(expr_arg) => {
                        // Only rewrite if it's a subquery, not the entire expression
                        // This preserves DISTINCT in COUNT(DISTINCT ...)
                        *expr_arg = rewrite_function_arg_expr(expr_arg.clone(), workspace_id);
                    }
                    FunctionArg::Named { arg, .. } => {
                        *arg = rewrite_function_arg_expr(arg.clone(), workspace_id);
                    }
                }
            }
            Expr::Function(new_func)
        }
        Expr::Nested(expr) => Expr::Nested(Box::new(rewrite_expr(*expr, workspace_id))),
        Expr::Case {
            operand,
            conditions,
            results,
            else_result,
        } => Expr::Case {
            operand: operand.map(|op| Box::new(rewrite_expr(*op, workspace_id))),
            conditions: conditions
                .into_iter()
                .map(|c| rewrite_expr(c, workspace_id))
                .collect(),
            results: results
                .into_iter()
                .map(|r| rewrite_expr(r, workspace_id))
                .collect(),
            else_result: else_result.map(|er| Box::new(rewrite_expr(*er, workspace_id))),
        },
        Expr::InSubquery {
            expr,
            subquery,
            negated,
        } => {
            // Rewrite the subquery in IN clause
            Expr::InSubquery {
                expr: Box::new(rewrite_expr(*expr, workspace_id)),
                subquery: rewrite_query(*subquery, workspace_id),
                negated,
            }
        }
        Expr::Exists { subquery, negated } => {
            // Rewrite the subquery in EXISTS clause
            Expr::Exists {
                subquery: rewrite_query(*subquery, workspace_id),
                negated,
            }
        }
        other => other, // Other expressions don't contain subqueries
    }
}

fn rewrite_function_arg_expr(expr: FunctionArgExpr, workspace_id: &str) -> FunctionArgExpr {
    match expr {
        FunctionArgExpr::Expr(expr) => FunctionArgExpr::Expr(rewrite_expr(expr, workspace_id)),
        other => other,
    }
}

fn rewrite_table_with_joins(twj: &mut TableWithJoins, workspace_id: &str) {
    // Rewrite subqueries in main relation
    if let TableFactor::Derived { subquery, .. } = &mut twj.relation {
        // Recursively rewrite the subquery by cloning and replacing
        let old_body = subquery.body.clone();
        *subquery.body = rewrite_set_expr(*old_body, workspace_id);
    }

    // Rewrite JOINs - add RLS filters to ON clauses for LEFT/RIGHT JOINs
    for join in &mut twj.joins {
        // Rewrite subqueries in JOIN relations
        if let TableFactor::Derived { subquery, .. } = &mut join.relation {
            let old_body = subquery.body.clone();
            *subquery.body = rewrite_set_expr(*old_body, workspace_id);
        }

        // Add RLS filter to JOIN ON clause if it's objects/edges table
        if is_objects_table(&join.relation) {
            // Get alias or use table name as alias
            let alias = get_table_alias(&join.relation)
                .or_else(|| get_table_name(&join.relation))
                .expect("Table should have name or alias");

            let workspace_filter = create_workspace_filter_with_alias(alias, workspace_id);

            // Add RLS filter to JOIN ON clause (for all JOIN types)
            match &mut join.join_operator {
                JoinOperator::LeftOuter(constraint)
                | JoinOperator::RightOuter(constraint)
                | JoinOperator::FullOuter(constraint)
                | JoinOperator::Inner(constraint) => {
                    match constraint {
                        JoinConstraint::On(existing_on) => {
                            // Add AND alias.__workspace = 'ws' to existing ON
                            let and_expr = Expr::BinaryOp {
                                left: Box::new(existing_on.clone()),
                                op: BinaryOperator::And,
                                right: Box::new(workspace_filter),
                            };
                            *constraint = JoinConstraint::On(and_expr);
                        }
                        _ => {
                            // Create new ON clause with RLS
                            *constraint = JoinConstraint::On(workspace_filter);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn is_objects_table(relation: &TableFactor) -> bool {
    match relation {
        TableFactor::Table { name, .. } => {
            let name_lower = name.to_string().to_lowercase();
            name_lower == "objects" || name_lower == "edges"
        }
        TableFactor::Derived {
            subquery,
            alias: _,
            lateral: _,
        } => {
            // Check subquery recursively
            match &*subquery.body {
                SetExpr::Select(select) => select
                    .from
                    .iter()
                    .any(|twj| is_objects_table(&twj.relation)),
                SetExpr::SetOperation { left, right, .. } => {
                    is_objects_in_set_expr(left) || is_objects_in_set_expr(right)
                }
                _ => false,
            }
        }
        TableFactor::TableFunction { .. }
        | TableFactor::Function { .. }
        | TableFactor::UNNEST { .. }
        | TableFactor::NestedJoin { .. }
        | TableFactor::Pivot { .. }
        | TableFactor::Unpivot { .. } => false,
    }
}

fn is_objects_in_set_expr(expr: &SetExpr) -> bool {
    match expr {
        SetExpr::Select(select) => select
            .from
            .iter()
            .any(|twj| is_objects_table(&twj.relation)),
        SetExpr::SetOperation { left, right, .. } => {
            is_objects_in_set_expr(left) || is_objects_in_set_expr(right)
        }
        _ => false,
    }
}

fn get_table_alias(relation: &TableFactor) -> Option<Ident> {
    match relation {
        TableFactor::Table { alias, .. } => alias.as_ref().map(|a| a.name.clone()),
        TableFactor::Derived { alias, .. } => alias.as_ref().map(|a| a.name.clone()),
        _ => None,
    }
}

fn get_table_name(relation: &TableFactor) -> Option<Ident> {
    match relation {
        TableFactor::Table { name, .. } => {
            // Use the last part of the table name as alias
            name.0.last().cloned()
        }
        _ => None,
    }
}

fn create_workspace_filter_with_alias(alias: Ident, workspace_id: &str) -> Expr {
    // Create: alias.__workspace = 'workspace_id'
    Expr::BinaryOp {
        left: Box::new(Expr::CompoundIdentifier(vec![
            alias,
            Ident::new("__workspace"),
        ])),
        op: BinaryOperator::Eq,
        right: Box::new(Expr::Value(Value::SingleQuotedString(
            workspace_id.to_string(),
        ))),
    }
}

fn create_workspace_filter_unqualified(workspace_id: &str) -> Expr {
    // Create: __workspace = 'workspace_id' (without alias - only for simple queries without JOINs)
    Expr::BinaryOp {
        left: Box::new(Expr::Identifier(Ident::new("__workspace"))),
        op: BinaryOperator::Eq,
        right: Box::new(Expr::Value(Value::SingleQuotedString(
            workspace_id.to_string(),
        ))),
    }
}

/// Format statements back to SQL string
/// This is a simplified formatter - sqlparser doesn't provide a built-in formatter
fn format_statements(statements: &[Statement]) -> String {
    statements
        .iter()
        .map(format_statement)
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_statement(stmt: &Statement) -> String {
    match stmt {
        Statement::Query(query) => format_query(query),
        other => format!("{:?}", other), // Fallback for other statement types
    }
}

fn format_query(query: &Query) -> String {
    let mut parts = Vec::new();

    // Format WITH clause if present
    if let Some(ref with) = query.with {
        if with.recursive {
            parts.push("WITH RECURSIVE".to_string());
        } else {
            parts.push("WITH".to_string());
        }
        let cte_parts: Vec<String> = with
            .cte_tables
            .iter()
            .map(|cte| {
                // Format CTE with optional column list
                let columns_str = if cte.alias.columns.is_empty() {
                    String::new()
                } else {
                    let cols: Vec<String> =
                        cte.alias.columns.iter().map(|c| c.to_string()).collect();
                    format!(" ({})", cols.join(", "))
                };
                format!(
                    "{}{} AS ({})",
                    cte.alias.name,
                    columns_str,
                    format_query(&cte.query)
                )
            })
            .collect();
        parts.push(cte_parts.join(", "));
    }

    // Format the main query body
    let body_str = format_set_expr(&query.body);
    parts.push(body_str);

    // Format ORDER BY
    if !query.order_by.is_empty() {
        let order_parts: Vec<String> = query
            .order_by
            .iter()
            .map(|item| {
                format!(
                    "{} {}",
                    format_expr(&item.expr),
                    if item.asc.unwrap_or(true) {
                        "ASC"
                    } else {
                        "DESC"
                    }
                )
            })
            .collect();
        parts.push(format!("ORDER BY {}", order_parts.join(", ")));
    }

    // Format LIMIT
    if let Some(ref limit) = query.limit {
        parts.push(format!("LIMIT {}", format_expr(limit)));
    }

    // Format OFFSET
    if let Some(ref offset) = query.offset {
        parts.push(format!("OFFSET {}", format_expr(&offset.value)));
    }

    parts.join(" ")
}

fn format_set_expr(expr: &SetExpr) -> String {
    match expr {
        SetExpr::Select(select) => format_select(select),
        SetExpr::SetOperation {
            op,
            left,
            right,
            set_quantifier,
        } => {
            let op_str = match op {
                SetOperator::Union => "UNION",
                SetOperator::Except => "EXCEPT",
                SetOperator::Intersect => "INTERSECT",
            };
            let all_str = match set_quantifier {
                SetQuantifier::All => " ALL",
                SetQuantifier::Distinct => " DISTINCT",
                _ => "", // Handle other variants
            };
            format!(
                "{} {}{} {}",
                format_set_expr(left),
                op_str,
                all_str,
                format_set_expr(right)
            )
        }
        SetExpr::Query(query) => format_query(query),
        other => format!("{:?}", other),
    }
}

fn format_select(select: &Select) -> String {
    let mut parts = Vec::new();

    // SELECT
    parts.push("SELECT".to_string());

    // DISTINCT
    if select.distinct.is_some() {
        parts.push("DISTINCT".to_string());
    }

    // Columns
    let columns: Vec<String> = select.projection.iter().map(format_projection).collect();
    parts.push(columns.join(", "));

    // FROM
    if !select.from.is_empty() {
        parts.push("FROM".to_string());
        let from_parts: Vec<String> = select.from.iter().map(format_table_with_joins).collect();
        parts.push(from_parts.join(", "));
    }

    // WHERE
    if let Some(where_expr) = &select.selection {
        parts.push("WHERE".to_string());
        parts.push(format_expr(where_expr));
    }

    // GROUP BY
    match &select.group_by {
        GroupByExpr::All => {
            parts.push("GROUP BY ALL".to_string());
        }
        GroupByExpr::Expressions(exprs) if !exprs.is_empty() => {
            parts.push("GROUP BY".to_string());
            let group_by: Vec<String> = exprs.iter().map(format_expr).collect();
            parts.push(group_by.join(", "));
        }
        _ => {}
    }

    // HAVING
    if let Some(ref having) = select.having {
        parts.push("HAVING".to_string());
        parts.push(format_expr(having));
    }

    parts.join(" ")
}

fn format_projection(proj: &SelectItem) -> String {
    match proj {
        SelectItem::UnnamedExpr(expr) => format_expr(expr),
        SelectItem::ExprWithAlias { expr, alias } => format!("{} AS {}", format_expr(expr), alias),
        SelectItem::Wildcard(_) => "*".to_string(),
        SelectItem::QualifiedWildcard(qualifier, _) => format!("{}.*", qualifier),
    }
}

fn format_table_with_joins(twj: &TableWithJoins) -> String {
    let mut result = format_table_factor(&twj.relation);

    for join in &twj.joins {
        let (join_type, constraint_opt) = match &join.join_operator {
            JoinOperator::Inner(constraint) => ("INNER", Some(constraint)),
            JoinOperator::LeftOuter(constraint) => ("LEFT", Some(constraint)), // LEFT JOIN is shorter than LEFT OUTER JOIN
            JoinOperator::RightOuter(constraint) => ("RIGHT", Some(constraint)), // RIGHT JOIN is shorter than RIGHT OUTER JOIN
            JoinOperator::FullOuter(constraint) => ("FULL OUTER", Some(constraint)),
            JoinOperator::CrossJoin => ("CROSS", None),
            JoinOperator::LeftSemi(_) => ("LEFT SEMI", None),
            JoinOperator::RightSemi(_) => ("RIGHT SEMI", None),
            JoinOperator::LeftAnti(_) => ("LEFT ANTI", None),
            JoinOperator::RightAnti(_) => ("RIGHT ANTI", None),
            JoinOperator::CrossApply => ("CROSS APPLY", None),
            JoinOperator::OuterApply => ("OUTER APPLY", None),
        };

        result.push_str(&format!(
            " {} JOIN {}",
            join_type,
            format_table_factor(&join.relation)
        ));

        // Format join constraint
        if let Some(constraint) = constraint_opt {
            match constraint {
                JoinConstraint::On(expr) => {
                    result.push_str(&format!(" ON {}", format_expr(expr)));
                }
                JoinConstraint::Using(columns) => {
                    let cols: Vec<String> = columns.iter().map(|c| c.to_string()).collect();
                    result.push_str(&format!(" USING ({})", cols.join(", ")));
                }
                JoinConstraint::Natural => {
                    result.push_str(" NATURAL");
                }
                _ => {}
            }
        }
    }

    result
}

fn format_table_factor(factor: &TableFactor) -> String {
    match factor {
        TableFactor::Table { name, alias, .. } => {
            let mut result = name.to_string();
            if let Some(alias) = alias {
                // Use space instead of AS for compatibility
                result.push_str(&format!(" {}", alias));
            }
            result
        }
        TableFactor::Derived {
            subquery,
            alias,
            lateral: _,
        } => {
            let mut result = format!("({})", format_query(subquery));
            if let Some(alias) = alias {
                // Use space instead of AS for compatibility
                result.push_str(&format!(" {}", alias));
            }
            result
        }
        TableFactor::Function { .. }
        | TableFactor::UNNEST { .. }
        | TableFactor::NestedJoin { .. }
        | TableFactor::Pivot { .. }
        | TableFactor::Unpivot { .. }
        | TableFactor::TableFunction { .. } => {
            format!("{:?}", factor) // Use Debug for complex variants
        }
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Identifier(ident) => ident.to_string(),
        Expr::CompoundIdentifier(idents) => idents
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("."),
        Expr::Value(value) => format_value(value),
        Expr::BinaryOp { left, op, right } => {
            format!(
                "{} {} {}",
                format_expr(left),
                format_binary_op(op),
                format_expr(right)
            )
        }
        Expr::UnaryOp { op, expr } => {
            format!("{}{}", format_unary_op(op), format_expr(expr))
        }
        Expr::Function(func) => format_function(func),
        Expr::Cast {
            expr,
            data_type,
            format: _,
        } => {
            format!(
                "CAST({} AS {})",
                format_expr(expr),
                format_data_type(data_type)
            )
        }
        Expr::Nested(expr) => format!("({})", format_expr(expr)),
        Expr::Subquery(query) => format!("({})", format_query(query)),
        Expr::Case {
            operand,
            conditions,
            results,
            else_result,
        } => {
            let mut result = "CASE".to_string();
            if let Some(op) = operand {
                result.push_str(&format!(" {}", format_expr(op)));
            }
            for (cond, res) in conditions.iter().zip(results.iter()) {
                result.push_str(&format!(
                    " WHEN {} THEN {}",
                    format_expr(cond),
                    format_expr(res)
                ));
            }
            if let Some(else_res) = else_result {
                result.push_str(&format!(" ELSE {}", format_expr(else_res)));
            }
            result.push_str(" END");
            result
        }
        // IN list: col IN ('a', 'b', 'c')
        Expr::InList {
            expr,
            list,
            negated,
        } => {
            let not_str = if *negated { "NOT " } else { "" };
            let items: Vec<String> = list.iter().map(format_expr).collect();
            format!("{} {}IN ({})", format_expr(expr), not_str, items.join(", "))
        }
        // IN subquery: col IN (SELECT ...)
        Expr::InSubquery {
            expr,
            subquery,
            negated,
        } => {
            let not_str = if *negated { "NOT " } else { "" };
            format!(
                "{} {}IN ({})",
                format_expr(expr),
                not_str,
                format_query(subquery)
            )
        }
        // EXISTS (SELECT ...)
        Expr::Exists { subquery, negated } => {
            let not_str = if *negated { "NOT " } else { "" };
            format!("{}EXISTS ({})", not_str, format_query(subquery))
        }
        // IS NULL / IS NOT NULL
        Expr::IsNull(expr) => format!("{} IS NULL", format_expr(expr)),
        Expr::IsNotNull(expr) => format!("{} IS NOT NULL", format_expr(expr)),
        // BETWEEN
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => {
            let not_str = if *negated { "NOT " } else { "" };
            format!(
                "{} {}BETWEEN {} AND {}",
                format_expr(expr),
                not_str,
                format_expr(low),
                format_expr(high)
            )
        }
        // LIKE
        Expr::Like {
            negated,
            expr,
            pattern,
            escape_char,
        } => {
            let not_str = if *negated { "NOT " } else { "" };
            let mut result = format!(
                "{} {}LIKE {}",
                format_expr(expr),
                not_str,
                format_expr(pattern)
            );
            if let Some(esc) = escape_char {
                result.push_str(&format!(" ESCAPE '{}'", esc));
            }
            result
        }
        // ILike (case-insensitive LIKE)
        Expr::ILike {
            negated,
            expr,
            pattern,
            escape_char,
        } => {
            let not_str = if *negated { "NOT " } else { "" };
            let mut result = format!(
                "{} {}ILIKE {}",
                format_expr(expr),
                not_str,
                format_expr(pattern)
            );
            if let Some(esc) = escape_char {
                result.push_str(&format!(" ESCAPE '{}'", esc));
            }
            result
        }
        // IS DISTINCT FROM
        Expr::IsDistinctFrom(left, right) => {
            format!(
                "{} IS DISTINCT FROM {}",
                format_expr(left),
                format_expr(right)
            )
        }
        Expr::IsNotDistinctFrom(left, right) => {
            format!(
                "{} IS NOT DISTINCT FROM {}",
                format_expr(left),
                format_expr(right)
            )
        }
        other => format!("{:?}", other),
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Number(n, _) => n.clone(),
        Value::SingleQuotedString(s) => format!("'{}'", s.replace('\'', "''")),
        Value::DoubleQuotedString(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        Value::Boolean(b) => b.to_string(),
        Value::Null => "NULL".to_string(),
        other => format!("{:?}", other),
    }
}

fn format_binary_op(op: &BinaryOperator) -> &str {
    match op {
        BinaryOperator::Plus => "+",
        BinaryOperator::Minus => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::StringConcat => "||",
        BinaryOperator::Gt => ">",
        BinaryOperator::Lt => "<",
        BinaryOperator::GtEq => ">=",
        BinaryOperator::LtEq => "<=",
        BinaryOperator::Eq => "=",
        BinaryOperator::NotEq => "!=",
        BinaryOperator::And => "AND",
        BinaryOperator::Or => "OR",
        // LIKE operators may not exist in this version
        BinaryOperator::BitwiseOr => "|",
        BinaryOperator::BitwiseAnd => "&",
        BinaryOperator::BitwiseXor => "^",
        BinaryOperator::PGBitwiseXor => "#",
        BinaryOperator::PGBitwiseShiftLeft => "<<",
        BinaryOperator::PGBitwiseShiftRight => ">>",
        BinaryOperator::Spaceship => "<=>",
        _ => "?",
    }
}

fn format_unary_op(op: &UnaryOperator) -> &str {
    match op {
        UnaryOperator::Plus => "+",
        UnaryOperator::Minus => "-",
        UnaryOperator::Not => "NOT ",
        UnaryOperator::PGBitwiseNot => "~",
        _ => "?",
    }
}

fn format_function(func: &Function) -> String {
    let name = func.name.to_string();
    let distinct_str = if func.distinct { "DISTINCT " } else { "" };
    let args: Vec<String> = func.args.iter().map(format_function_arg).collect();
    let mut result = format!("{}({}{})", name, distinct_str, args.join(", "));

    // OVER clause for window functions
    if let Some(ref over) = func.over {
        result.push_str(" OVER (");
        let mut over_parts = Vec::new();

        match over {
            WindowType::WindowSpec(spec) => {
                // PARTITION BY
                if !spec.partition_by.is_empty() {
                    let partition: Vec<String> =
                        spec.partition_by.iter().map(format_expr).collect();
                    over_parts.push(format!("PARTITION BY {}", partition.join(", ")));
                }

                // ORDER BY
                if !spec.order_by.is_empty() {
                    let order: Vec<String> =
                        spec.order_by.iter().map(format_order_by_expr).collect();
                    over_parts.push(format!("ORDER BY {}", order.join(", ")));
                }
            }
            WindowType::NamedWindow(name) => {
                over_parts.push(name.to_string());
            }
        }

        result.push_str(&over_parts.join(" "));
        result.push(')');
    }

    result
}

fn format_order_by_expr(order: &OrderByExpr) -> String {
    let mut result = format_expr(&order.expr);
    if let Some(asc) = order.asc {
        result.push_str(if asc { " ASC" } else { " DESC" });
    }
    if let Some(nulls_first) = order.nulls_first {
        result.push_str(if nulls_first {
            " NULLS FIRST"
        } else {
            " NULLS LAST"
        });
    }
    result
}

fn format_function_arg(arg: &FunctionArg) -> String {
    match arg {
        FunctionArg::Unnamed(expr) => format_function_arg_expr(expr),
        FunctionArg::Named { name, arg } => {
            format!("{} => {}", name, format_function_arg_expr(arg))
        }
    }
}

fn format_function_arg_expr(expr: &FunctionArgExpr) -> String {
    match expr {
        FunctionArgExpr::Expr(expr) => format_expr(expr),
        FunctionArgExpr::QualifiedWildcard(qualifier) => format!("{}.*", qualifier),
        FunctionArgExpr::Wildcard => "*".to_string(),
    }
}

fn format_data_type(dt: &DataType) -> String {
    format!("{:?}", dt)
}

/// Three-way classification of a SQL string's read-only status, used by the read-only guard.
///
/// `sqlparser`'s grammar coverage is **not** identical to SQLite's (e.g. it has no `GLOB`/`MATCH`
/// operator). So a parse failure is NOT evidence of a write — it just means we can't statically
/// confirm the shape. The guard treats the three cases differently (see `core::sql_guard`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOnlyClass {
    /// Parsed, and every statement is a read-only `Statement::Query` (SELECT-class).
    ReadOnly,
    /// Parsed, and at least one statement writes / is not a query. Carries a stable reason.
    Writable(String),
    /// Could not be parsed by `sqlparser`. Not evidence of a write — the caller should defer
    /// to the engine-level guard (SQLite `PRAGMA query_only`). Carries the parser error.
    Unparseable(String),
}

/// Classify whether the given SQL is read-only (SELECT-class), writable, or unparseable.
///
/// Uses `sqlparser` to parse and inspect the AST. Only `Statement::Query` is admitted as
/// read-only; INSERT/UPDATE/DELETE/CREATE/DROP/ALTER/ATTACH/PRAGMA/etc. are `Writable`. Inputs
/// `sqlparser` cannot parse are `Unparseable` (its grammar ⊂ SQLite's).
pub fn classify_read_only(sql: &str) -> ReadOnlyClass {
    let dialect = GenericDialect {};
    let statements = match Parser::parse_sql(&dialect, sql) {
        Ok(s) => s,
        Err(e) => {
            // sqlparser couldn't parse it (its grammar ⊂ SQLite's). Only defer to the engine
            // guard when the SQL actually *starts as a read query*; otherwise (PRAGMA, ATTACH,
            // an unparseable write, …) classify it Writable so the parser layer rejects it
            // rather than letting it reach SQLite. The engine `query_only` is still the backstop.
            if starts_as_read_query(sql) {
                return ReadOnlyClass::Unparseable(format!("{}", e));
            }
            return ReadOnlyClass::Writable("statement is not a read-only query".to_string());
        }
    };

    if statements.is_empty() {
        return ReadOnlyClass::Writable("empty SQL statement".to_string());
    }

    for stmt in &statements {
        if !matches!(stmt, Statement::Query(_)) {
            // Stable, content-free reason: the AST *variant name* only (e.g. "Delete"), never
            // the full Debug repr — which embeds the user's SQL and changes across sqlparser
            // versions.
            let debug = format!("{:?}", stmt);
            let kind: String = debug.chars().take_while(|c| c.is_alphanumeric()).collect();
            let kind = if kind.is_empty() {
                "non-select".to_string()
            } else {
                kind
            };
            return ReadOnlyClass::Writable(format!("statement is not read-only: {}", kind));
        }
    }

    ReadOnlyClass::ReadOnly
}

/// Whether `sql` begins as a read-only query — its first keyword is SELECT/WITH/EXPLAIN/VALUES
/// (after leading whitespace and `(`). Used to decide whether an *unparseable* statement may be
/// deferred to the engine guard. A `WITH`-prefixed write still gets caught by the engine
/// `query_only` backstop, so this only narrows the surface, it isn't the sole defense.
fn starts_as_read_query(sql: &str) -> bool {
    let s = sql.trim_start_matches(|c: char| c.is_whitespace() || c == '(');
    let word: String = s.chars().take_while(|c| c.is_alphabetic()).collect();
    matches!(
        word.to_ascii_uppercase().as_str(),
        "SELECT" | "WITH" | "EXPLAIN" | "VALUES"
    )
}

/// Back-compat boolean-style check: read-only ⇒ `Ok`, writable or unparseable ⇒ `Err`.
/// Prefer [`classify_read_only`] when you need to distinguish a parse failure from a write.
pub fn is_read_only(sql: &str) -> Result<(), String> {
    match classify_read_only(sql) {
        ReadOnlyClass::ReadOnly => Ok(()),
        ReadOnlyClass::Writable(reason) => Err(reason),
        ReadOnlyClass::Unparseable(e) => Err(format!("SQL parse error: {}", e)),
    }
}

#[cfg(test)]
mod read_only_tests {
    use super::{classify_read_only, ReadOnlyClass};

    #[test]
    fn plain_select_is_read_only() {
        assert_eq!(
            classify_read_only("SELECT __id FROM objects"),
            ReadOnlyClass::ReadOnly
        );
    }

    #[test]
    fn writes_are_classified_writable() {
        // Parseable non-query statements must be positively rejected (not deferred).
        for sql in [
            "DELETE FROM objects",
            "UPDATE objects SET __id='x'",
            "INSERT INTO objects(__id) VALUES('x')",
            "DROP TABLE objects",
            "ATTACH DATABASE 'x.db' AS y",
            "PRAGMA query_only=OFF",
        ] {
            assert!(
                matches!(classify_read_only(sql), ReadOnlyClass::Writable(_)),
                "expected Writable for: {sql}"
            );
        }
    }

    #[test]
    fn sqlite_only_syntax_is_unparseable_not_writable() {
        // `GLOB` is valid read-only SQLite but unknown to sqlparser 0.40. It must NOT be
        // classified as a write (that was the bug: a parse error mislabeled as not-read-only).
        // It is deferred to the engine `query_only` guard instead.
        let read_only_glob =
            "SELECT key FROM objects, json_each(objects.data) WHERE key GLOB 'qmd[0-9]*'";
        assert!(
            matches!(
                classify_read_only(read_only_glob),
                ReadOnlyClass::Unparseable(_)
            ),
            "GLOB select should be Unparseable, got {:?}",
            classify_read_only(read_only_glob)
        );

        // An unparseable WRITE (UPDATE ... GLOB ...) does NOT start as a read query, so it is
        // classified Writable and rejected at the parser layer — it never reaches the engine.
        let unparseable_write = "UPDATE objects SET __id='x' WHERE __id GLOB 'q*'";
        assert!(matches!(
            classify_read_only(unparseable_write),
            ReadOnlyClass::Writable(_)
        ));
    }
}
