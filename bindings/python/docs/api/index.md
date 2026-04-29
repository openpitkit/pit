# Python API reference

This reference documents the public Python layer. It is generated with Sphinx
`autodoc` from the `openpit` package and is organized by import path.

## Import layout

Use these import paths in application code:

- `openpit`: engine, root order/report models, and commonly used classes.
- `openpit.param`: domain value types and enums.
- `openpit.pretrade`: result handles, rejects, and policy interfaces.
- `openpit.pretrade.policies`: built-in policies.
- `openpit.account_adjustment`: account-adjustment models and policy interface.

## Engine

```{eval-rst}
.. automodule:: openpit
   :no-index:

.. autoclass:: openpit.Engine
   :members:

.. autoclass:: openpit.EngineBuilder
   :members:

.. autoclass:: openpit.RejectError
   :members:
```

## Domain value types

The `openpit.param` module contains identifiers, enums, and exact
financial-domain values. These objects should be used at API boundaries instead
of raw strings and numbers.

```{eval-rst}
.. automodule:: openpit.param
   :members:
   :undoc-members:
```

## Core order and report models

Core models group order, execution-report, and mutation payloads. `Order` and
`ExecutionReport` are intentionally extensible: applications may subclass them
to carry project metadata into policy callbacks.

```{eval-rst}
.. automodule:: openpit.core
   :members:
   :undoc-members:
```

## Pre-trade results and handles

These classes model the two-stage pre-trade lifecycle. `StartPreTradeResult`
contains a deferred request handle. `ExecuteResult` contains either rejects or a
reservation that must be committed or rolled back.

```{eval-rst}
.. automodule:: openpit.pretrade

.. autoclass:: openpit.pretrade.PreTradeContext
   :members:

.. autoclass:: openpit.pretrade.PreTradeLock
   :members:

.. autoclass:: openpit.pretrade.RejectCode
   :members:

.. autoclass:: openpit.pretrade.RejectScope
   :members:

.. autoclass:: openpit.pretrade.StartPreTradeResult
   :members:

.. autoclass:: openpit.pretrade.ExecuteResult
   :members:

.. autoclass:: openpit.pretrade.PostTradeResult
   :members:

.. autoclass:: openpit.pretrade.AccountAdjustmentBatchResult
   :members:

.. autoclass:: openpit.pretrade.PreTradeRequest
   :members:

.. autoclass:: openpit.pretrade.PreTradeReservation
   :members:

.. autoclass:: openpit.pretrade.Reject
   :members:
```

## Policy interfaces

Use these interfaces to implement custom business checks. Policy rejects are
normal outcomes and should be returned, not raised.

```{eval-rst}
.. automodule:: openpit.pretrade.policy
   :members:
   :undoc-members:
```

## Built-in policies

Built-in policies are registered through `EngineBuilder` like custom policies.
They produce standard `RejectCode` values.

```{eval-rst}
.. automodule:: openpit.pretrade.policies
   :members:
   :undoc-members:
```

## Account adjustments

Account-adjustment models and policies validate administrative balance or
position changes outside the normal order flow.

```{eval-rst}
.. automodule:: openpit.account_adjustment
   :members:
   :undoc-members:
```
