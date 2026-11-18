"""Writes tiny synthetic background removal models (ONNX) for the harness:
no real model is downloaded or bundled. Each maps the normalised image to a
"mask" with plain operators, covering the shapes ShareX's
BackgroundRemovalService handles: NCHW and NHWC inputs, a dynamic input
(ShareX's 1024x1024 default), 4-D outputs channels first and last, a 2-D
output, and masks with negative values (ShareX's sigmoid branch) or not
(min-max normalisation).

    python tools/tools-harness/make_bg_models.py <out dir>

Needs the `onnx` Python package (only to write the files).
"""

import os
import sys

import onnx
from onnx import TensorProto, helper

out = sys.argv[1]
os.makedirs(out, exist_ok=True)


def const(name, values, dims=None):
    return helper.make_node("Constant", [], [name], value=helper.make_tensor(name + "_v", TensorProto.FLOAT,
                                                                             dims or [len(values)], values))


def save(name, nodes, inp, outp):
    graph = helper.make_graph(nodes, name, [inp], [outp])
    model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)], producer_name="sharexrs-harness")
    model.ir_version = 7
    onnx.checker.check_model(model)
    onnx.save(model, os.path.join(out, name + ".onnx"))


# a: NCHW 32x32, mean over channels minus 0.3 (negatives: sigmoid).
save("a-nchw-sigmoid", [
    helper.make_node("ReduceMean", ["input"], ["m"], axes=[1], keepdims=1),
    const("k", [0.3]),
    helper.make_node("Sub", ["m", "k"], ["mask"]),
], helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 3, 32, 32]),
    helper.make_tensor_value_info("mask", TensorProto.FLOAT, [1, 1, 32, 32]))

# b: NCHW 24x40, a weighted channel sum through a sigmoid (all positive: min-max).
save("b-nchw-minmax", [
    const("w", [0.9, -1.3, 0.6], [1, 3, 1, 1]),
    helper.make_node("Mul", ["input", "w"], ["p"]),
    # Opset 13's ReduceSum takes its axes as an input.
    helper.make_node("Constant", [], ["axes"], value=helper.make_tensor("axes_v", TensorProto.INT64, [1], [1])),
    helper.make_node("ReduceSum", ["p", "axes"], ["s"], keepdims=1),
    helper.make_node("Sigmoid", ["s"], ["mask"]),
], helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 3, 24, 40]),
    helper.make_tensor_value_info("mask", TensorProto.FLOAT, [1, 1, 24, 40]))

# c: NHWC 20x28 in, channels-last mask out, negatives.
save("c-nhwc", [
    helper.make_node("ReduceMax", ["input"], ["m"], axes=[3], keepdims=1),
    const("k", [0.5]),
    helper.make_node("Sub", ["m", "k"], ["mask"]),
], helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 20, 28, 3]),
    helper.make_tensor_value_info("mask", TensorProto.FLOAT, [1, 20, 28, 1]))

# d: dynamic NCHW (1024x1024 by default), a 2-D mask.
save("d-dynamic-2d", [
    helper.make_node("ReduceMin", ["input"], ["mask"], axes=[0, 1], keepdims=0),
], helper.make_tensor_value_info("input", TensorProto.FLOAT, ["n", 3, "h", "w"]),
    helper.make_tensor_value_info("mask", TensorProto.FLOAT, ["h", "w"]))

print("models written to", out)
