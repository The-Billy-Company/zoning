We now trace Python 3.15's explicit lazy imports through the same dependency graph as eager
imports. Deferring a module no longer hides its import boundaries, dependency grants or cycles
from verification.
