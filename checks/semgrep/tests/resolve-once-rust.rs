// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

fn unbounded_cache_scans(cache: Cache) {
    // ruleid: operation-cache-keyed-lookup
    cache.instruments.iter().find(|entry| entry.pair == pair);
    // ruleid: operation-cache-keyed-lookup
    cache.quotes.iter().find(|entry| entry.key == query);
    // ruleid: operation-cache-keyed-lookup
    cache.services.iter().find_map(|entry| entry.value());
    // ruleid: operation-cache-keyed-lookup
    cache.quotes.iter().position(|entry| entry.key == query);
    // ok: operation-cache-keyed-lookup
    cache.instruments.get(&pair);
    // ok: operation-cache-keyed-lookup
    cache.quotes.get(&query);
    let entries = &cache.quotes;
    // ruleid: operation-cache-keyed-lookup
    entries.iter().find(|entry| entry.key == query);
}

fn custom_policy(ctx: &PreTradeContext<Factory>, accounts: Accounts, order: Order, report: Report) {
    // ruleid: operation-account-data-from-context
    accounts.group_of(account);
    // ruleid: operation-account-data-from-context
    accounts.currency_of(account);
    // ruleid: operation-account-data-from-context
    accounts.currency_of_in_group(account, group);
    // ruleid: operation-account-read-once
    order.account_id();
    // ruleid: operation-account-read-once
    report.account_id();
    // ok: operation-account-read-once
    ctx.account_id();
    // ok: operation-account-data-from-context
    ctx.account_group();
}

fn mixed_context_and_renamed_payload(
    ctx: &PostTradeContext<Factory>,
    execution_report: Report,
) -> Option<Result> {
    // ok: operation-account-read-once
    ctx.account_id();
    // ruleid: operation-account-read-once
    execution_report.account_id();
    let event = execution_report;
    // ruleid: operation-account-read-once
    event.account_id();
    let request = event;
    // ruleid: operation-account-read-once
    request.account_id();
}

fn misleading_context_name(ctx: Report) -> Option<Result> {
    // ruleid: operation-account-read-once
    ctx.account_id();
}

fn apply_execution_report(ctx: Context, report: Report) -> Option<PostTradeResult> {
    // ruleid: operation-account-read-once
    report.account_id();
}

impl<F> SharedOperationSnapshot<F> {
    fn account_group(&self) -> Option<Group> {
        // ok: operation-account-data-from-context
        self.source.handle.group_of(account)
    }
}

impl<F> OperationSnapshot<F> {
    fn account_id(&self) -> Result<Account, Error> {
        // ok: operation-account-read-once
        self.source.account_id()
    }
}

fn with_blocking_cause<Output>(groups: Groups) -> Option<Output> {
    // ok: operation-account-data-from-context
    groups.group_of(account)
}

fn start_pre_trade(&self, order: Trait::Order) -> Result<Request, Error> {
    // ok: operation-account-read-once
    order.account_id()
}

mod tests {
    fn cache_scan_fixture(cache: Cache) {
        // ok: operation-cache-keyed-lookup
        cache.quotes.iter().find(|entry| entry.key == query);
    }
    fn account_accessor_test(report: Report, accounts: Accounts) {
        // ok: operation-account-read-once
        report.account_id();
        // ok: operation-account-data-from-context
        accounts.group_of(account);
    }
}
