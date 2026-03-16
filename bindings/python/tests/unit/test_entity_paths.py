import openpit
from openpit.pretrade import policies


def test_pretrade_entities_have_rust_like_module_paths() -> None:
    assert hasattr(openpit, "Instrument")
    assert hasattr(openpit, "Order")
    assert hasattr(openpit, "ExecutionReport")
    assert hasattr(openpit, "PostTradeResult")
    assert not hasattr(openpit, "Request")
    assert not hasattr(openpit, "Reservation")
    assert not hasattr(openpit, "Reject")
    assert not hasattr(openpit, "RejectCode")

    assert not hasattr(openpit.pretrade, "Order")
    assert openpit.Instrument.__module__ == "openpit.core"
    assert openpit.Order.__module__ == "openpit.core"
    assert openpit.ExecutionReport.__module__ == "openpit.core"
    assert openpit.pretrade.Request.__module__ == "openpit.pretrade"
    assert openpit.pretrade.Reservation.__module__ == "openpit.pretrade"
    assert not hasattr(openpit.pretrade, "ExecutionReport")
    assert openpit.pretrade.Reject.__module__ == "openpit.pretrade"
    assert openpit.pretrade.RejectCode.__module__ == "openpit.pretrade"
    assert openpit.PostTradeResult.__module__ == "openpit.pretrade"


def test_builtins_policies_have_rust_like_module_paths() -> None:
    assert not hasattr(openpit, "OrderValidationPolicy")
    assert not hasattr(openpit, "RateLimitPolicy")
    assert not hasattr(openpit, "PnlKillSwitchPolicy")
    assert not hasattr(openpit, "OrderSizeLimit")
    assert not hasattr(openpit, "OrderSizeLimitPolicy")

    assert policies.OrderValidationPolicy.__module__ == "openpit.pretrade.policies"
    assert policies.RateLimitPolicy.__module__ == "openpit.pretrade.policies"
    assert policies.PnlKillSwitchPolicy.__module__ == "openpit.pretrade.policies"
    assert policies.OrderSizeLimit.__module__ == "openpit.pretrade.policies"
    assert policies.OrderSizeLimitPolicy.__module__ == "openpit.pretrade.policies"
