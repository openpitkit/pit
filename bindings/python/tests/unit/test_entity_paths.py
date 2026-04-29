import openpit
from openpit.pretrade import policies


def test_pretrade_entities_have_rust_like_module_paths() -> None:
    assert hasattr(openpit, "Instrument")
    assert hasattr(openpit, "Order")
    assert hasattr(openpit, "ExecutionReport")
    assert hasattr(openpit, "PostTradeResult")
    assert hasattr(openpit, "Mutation")
    assert not hasattr(openpit, "PreTradeRequest")
    assert not hasattr(openpit, "PreTradeReservation")
    assert not hasattr(openpit, "Reject")
    assert not hasattr(openpit, "RejectCode")

    assert not hasattr(openpit.pretrade, "Order")
    assert openpit.Instrument.__module__ == "openpit.core"
    assert openpit.Order.__module__ == "openpit.core"
    assert openpit.ExecutionReport.__module__ == "openpit.core"
    assert openpit.Mutation.__module__ == "openpit.core"
    assert openpit.pretrade.PreTradeRequest.__module__ == "openpit.pretrade"
    assert openpit.pretrade.PreTradeReservation.__module__ == "openpit.pretrade"
    assert not hasattr(openpit.pretrade, "ExecutionReport")
    assert not hasattr(openpit.pretrade, "Mutation")
    assert openpit.pretrade.Reject.__module__ == "openpit.pretrade"
    assert openpit.pretrade.RejectCode.__module__ == "openpit.pretrade"
    assert openpit.PostTradeResult.__module__ == "openpit.pretrade"


def test_builtins_policies_have_rust_like_module_paths() -> None:
    assert not hasattr(openpit, "OrderValidationPolicy")
    assert not hasattr(openpit, "RateLimitPolicy")
    assert not hasattr(openpit, "PnlBoundsKillSwitchPolicy")
    assert not hasattr(openpit, "OrderSizeLimit")
    assert not hasattr(openpit, "OrderSizeLimitPolicy")

    assert policies.OrderValidationPolicy.__module__ == "openpit.pretrade.policies"
    assert policies.RateLimitPolicy.__module__ == "openpit.pretrade.policies"
    assert policies.PnlBoundsKillSwitchPolicy.__module__ == "openpit.pretrade.policies"
    assert policies.OrderSizeLimit.__module__ == "openpit.pretrade.policies"
    assert policies.OrderSizeLimitPolicy.__module__ == "openpit.pretrade.policies"
