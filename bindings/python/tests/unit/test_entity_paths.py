import openpit
from openpit.pretrade import policies


def test_pretrade_entities_have_rust_like_module_paths() -> None:
    assert hasattr(openpit, "Instrument")
    assert hasattr(openpit, "Order")
    assert hasattr(openpit, "ExecutionReport")
    assert hasattr(openpit, "PostTradeResult")
    assert hasattr(openpit, "Mutation")
    assert not hasattr(openpit, "Request")
    assert not hasattr(openpit, "Reservation")
    assert not hasattr(openpit, "Reject")
    assert not hasattr(openpit, "RejectCode")

    assert not hasattr(openpit.pretrade, "Order")
    assert openpit.Instrument.__module__ == "openpit.core"
    assert openpit.Order.__module__ == "openpit.core"
    assert openpit.ExecutionReport.__module__ == "openpit.core"
    assert openpit.Mutation.__module__ == "openpit.core"
    assert openpit.pretrade.Request.__module__ == "openpit.pretrade"
    assert openpit.pretrade.Reservation.__module__ == "openpit.pretrade"
    assert not hasattr(openpit.pretrade, "PreTradeRequest")
    assert not hasattr(openpit.pretrade, "PreTradeReservation")
    assert not hasattr(openpit.pretrade, "ExecutionReport")
    assert not hasattr(openpit.pretrade, "Mutation")
    assert openpit.pretrade.Reject.__module__ == "openpit.pretrade"
    assert openpit.pretrade.RejectCode.__module__ == "openpit.pretrade"
    assert openpit.PostTradeResult.__module__ == "openpit.pretrade"
    assert openpit.pretrade.PostTradeResult is openpit.PostTradeResult
    assert openpit.pretrade.AccountBlock.__module__ == "openpit.pretrade"
    assert openpit.pretrade.OutcomeAmount.__module__ == "openpit.pretrade"
    assert openpit.pretrade.AccountOutcomeEntry.__module__ == "openpit.pretrade"
    assert openpit.pretrade.AccountAdjustmentOutcome.__module__ == "openpit.pretrade"

    assert openpit.account_adjustment.Adjustment.__module__ == (
        "openpit.account_adjustment"
    )
    assert openpit.account_adjustment.Amount.__module__ == "openpit.account_adjustment"
    assert not hasattr(openpit.account_adjustment, "AccountAdjustment")
    assert not hasattr(openpit.account_adjustment, "AccountAdjustmentAmount")


def test_builtins_policies_have_rust_like_module_paths() -> None:
    assert not hasattr(openpit, "OrderSizeLimit")
    assert not hasattr(openpit, "PnlBoundsBrokerBarrier")
    assert not hasattr(openpit, "PnlBoundsAccountAssetBarrier")

    assert policies.OrderSizeLimit.__module__ == "openpit.pretrade.policies"
    assert policies.PnlBoundsBrokerBarrier.__module__ == "openpit.pretrade.policies"
    assert (
        policies.PnlBoundsAccountAssetBarrier.__module__ == "openpit.pretrade.policies"
    )
