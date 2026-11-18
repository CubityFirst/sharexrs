"""Writes analyze_cases.json: Analyze image's requests and answers.

Each case runs AnalyzeImageService (analyze, test connection or load models)
with made-up AIOptions against canned answers on the mock server
(tools/uploaders-harness/mock_server.py). The keys are fake and every request
goes to 127.0.0.1. Images come from the harness's analyze-inputs folder
(AnalyzeCases.cs writes them with ShareX's encoders).

"expect_differ" marks a case where ShareXrs differs from ShareX on purpose.
"""

import json
import sys

BS = chr(92)

BASE_OPTIONS = {
    "Provider": "OpenAI",
    "OpenAIAPIKey": "sk-test-openai",
    "OpenAIModel": "gpt-5-mini",
    "OpenAICustomURL": None,
    "OpenAIReasoningEffort": "minimal",
    "OpenAIVerbosity": "medium",
    "GeminiAPIKey": "gm-test-key",
    "GeminiModel": "gemini-2.0-flash",
    "OpenRouterAPIKey": "or-test-key",
    "OpenRouterModel": "openai/gpt-4o",
    "Input": "What is in this image?",
    "AutoStartRegion": True,
    "AutoStartAnalyze": True,
    "AutoCopyResult": False,
}


def opts(provider="OpenAI", **kw):
    o = dict(BASE_OPTIONS)
    o["Provider"] = provider
    o.update(kw)
    return o


def ok(body, content_type="application/json; charset=utf-8"):
    text = body if isinstance(body, str) else json.dumps(body)
    return {"status": 200, "reason": "OK", "headers": [["Content-Type", content_type]], "body": text}


def status(code, reason, body="", content_type="application/json; charset=utf-8"):
    return {"status": code, "reason": reason, "headers": [["Content-Type", content_type]], "body": body}


OPENAI_OK = ok({
    "id": "resp_1",
    "object": "response",
    "output": [
        {"id": "rs_1", "type": "reasoning", "summary": [], "content": None},
        {"id": "msg_1", "type": "message", "status": "completed", "role": "assistant",
         "content": [{"type": "output_text", "annotations": [], "text": "A gradient of colours."}]},
    ],
})
LEGACY_OK = ok({"id": "chatcmpl-1", "object": "chat.completion",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "A gradient."}, "finish_reason": "stop"}]})
GEMINI_OK = ok({"candidates": [{"content": {"parts": [{"text": "A colourful gradient."}], "role": "model"}, "finishReason": "STOP"}]})
OPENROUTER_OK = ok({"id": "gen-1", "choices": [{"message": {"role": "assistant", "content": "Gradient."}}]})
OK_FOR = {"OpenAI": OPENAI_OK, "OpenAILegacy": LEGACY_OK, "Gemini": GEMINI_OK, "OpenRouter": OPENROUTER_OK}

cases = []


def analyze(id, options, response=None, file=None, data_file=None, empty_data=False, **extra):
    case = {"id": id, "kind": "analyze", "options": options, "responses": [response] if response else []}
    if file:
        case["file"] = file
    if data_file:
        case["data_file"] = data_file
    if empty_data:
        case["empty_data"] = True
    case.update(extra)
    cases.append(case)


# Every provider, every kind of input.
for provider in ["OpenAI", "OpenAILegacy", "Gemini", "OpenRouter"]:
    for source, kw in [("file_png", {"file": "opaque.png"}), ("file_jpg", {"file": "photo.jpg"}),
                       ("data_png", {"data_file": "opaque.png"}), ("data_alpha", {"data_file": "alpha.png"})]:
        analyze(f"{provider}_{source}", opts(provider), OK_FOR[provider], **kw)
    analyze(f"{provider}_empty_data", opts(provider), empty_data=True)
    analyze(f"{provider}_not_an_image", opts(provider), OK_FOR[provider], data_file="not-an-image.bin")

# Prompts: escaping in both JSON writers, empty and null prompts.
tricky = 'Describe "this" <img> & it\'s +1 `x` ' + BS + ' é ü 中文 😀 \n\t' + chr(0x2028) + ' / end'
for provider in ["OpenAI", "OpenAILegacy", "Gemini", "OpenRouter"]:
    analyze(f"{provider}_prompt_tricky", opts(provider, Input=tricky), OK_FOR[provider], data_file="opaque.png")
    analyze(f"{provider}_prompt_empty", opts(provider, Input=""), OK_FOR[provider], data_file="opaque.png")
    analyze(f"{provider}_prompt_null", opts(provider, Input=None), OK_FOR[provider], data_file="opaque.png")

# OpenAI's reasoning effort and verbosity rules, models, URLs, keys.
for i, (model, effort) in enumerate([("gpt-5.1", "minimal"), ("gpt-5.1", "high"), ("GPT-5.2", None), ("gpt-5", None),
                                     ("gpt-5-mini", "low"), ("gpt-5.", "MINIMAL"), ("o4-mini", "medium")]):
    analyze(f"OpenAI_effort_{i}", opts(OpenAIModel=model, OpenAIReasoningEffort=effort), OPENAI_OK, data_file="opaque.png")
analyze("OpenAI_verbosity_null", opts(OpenAIVerbosity=None), OPENAI_OK, data_file="opaque.png")
analyze("OpenAI_verbosity_high", opts(OpenAIVerbosity="high"), OPENAI_OK, data_file="opaque.png")
analyze("OpenAI_model_null", opts(OpenAIModel=None), OPENAI_OK, data_file="opaque.png")
analyze("OpenAILegacy_model_null", opts("OpenAILegacy", OpenAIModel=None), LEGACY_OK, data_file="opaque.png")
for i, url in enumerate(["http://localhost:1234", "https://proxy.example/v1/responses", "https://proxy.example/base/",
                         "https://proxy.example/v1/chat/completions", ""]):
    analyze(f"OpenAI_custom_url_{i}", opts(OpenAICustomURL=url), OPENAI_OK, data_file="opaque.png")
    analyze(f"OpenAILegacy_custom_url_{i}", opts("OpenAILegacy", OpenAICustomURL=url), LEGACY_OK, data_file="opaque.png")
analyze("OpenAI_key_special", opts(OpenAIAPIKey="sk-a+b/c=d"), OPENAI_OK, data_file="opaque.png")
analyze("OpenAI_key_null", opts(OpenAIAPIKey=None), OPENAI_OK, data_file="opaque.png")
analyze("Gemini_key_special", opts("Gemini", GeminiAPIKey="gm key+/?&=x"), GEMINI_OK, data_file="opaque.png")
analyze("Gemini_model_special", opts("Gemini", GeminiModel="models/gemini 2.0"), GEMINI_OK, data_file="opaque.png")
analyze("Gemini_model_null", opts("Gemini", GeminiModel=None), GEMINI_OK, data_file="opaque.png")
analyze("OpenRouter_model_null", opts("OpenRouter", OpenRouterModel=None), OPENROUTER_OK, data_file="opaque.png")
analyze("OpenRouter_key_null", opts("OpenRouter", OpenRouterAPIKey=None), OPENROUTER_OK, data_file="opaque.png")
analyze("Gemini_missing_file", opts("Gemini"), GEMINI_OK, file="missing.png")

# Answers: OpenAI (System.Text.Json, case-sensitive and strict).
openai_answers = {
    "empty_output": ok({"output": []}),
    "null_output": ok({"output": None}),
    "texts_empty": ok({"output": [{"content": [{"text": ""}]}, {"content": [{"text": None}]}]}),
    "second_has_text": ok({"output": [{"content": []}, {"content": [{"text": "second"}, {"text": "third"}]}]}),
    "first_content_null": ok({"output": [{"content": [None]}]}),
    "output_item_null": ok({"output": [None]}),
    "root_null": ok("null"),
    "root_array": ok("[]"),
    "malformed": ok('{"output": ['),
    "empty_body": ok(""),
    "id_number": ok({"id": 5, "output": []}),
    "output_object": ok({"output": {}}),
    "text_number": ok({"output": [{"content": [{"text": 5}]}]}),
    "case_sensitive": ok({"Output": [{"content": [{"text": "x"}]}]}),
    "unicode_and_lines": ok('{"output":[{"content":[{"text":"line 1\\nline 2\\r\\n' + BS + 'u00e9 ' + BS + 'ud83d' + BS + 'ude00"}]}]}'),
    "bom": ok(chr(0xFEFF) + json.dumps({"output": [{"content": [{"text": "bom"}]}]})),
    "http_401": status(401, "Unauthorized", '{"error":{"message":"Incorrect API key provided"}}'),
    "http_429": status(429, "Too Many Requests", '{"error":{}}'),
    "http_500_empty": status(500, "Internal Server Error"),
}
for name, response in openai_answers.items():
    analyze(f"OpenAI_answer_{name}", opts(), response, data_file="opaque.png")

legacy_answers = {
    "choices_empty": ok({"choices": []}),
    "message_null": ok({"choices": [{"message": None}]}),
    "content_null": ok({"choices": [{"message": {"content": None}}]}),
    "choice_null": ok({"choices": [None]}),
    "root_null": ok("null"),
    "content_number": ok({"choices": [{"message": {"content": 1}}]}),
    "http_401": status(401, "Unauthorized", "{}"),
}
for name, response in legacy_answers.items():
    analyze(f"OpenAILegacy_answer_{name}", opts("OpenAILegacy"), response, data_file="opaque.png")

# Gemini and OpenRouter (Newtonsoft through dynamic).
dynamic_answers = {
    "Gemini": {
        "http_400": status(400, "Bad Request", '{"error":{"code":400,"message":"API key not valid."}}'),
        "no_candidates": ok({}),
        "candidates_empty": ok({"candidates": []}),
        "candidates_object": ok({"candidates": {}}),
        "text_number": ok({"candidates": [{"content": {"parts": [{"text": 5}]}}]}),
        "text_bool": ok({"candidates": [{"content": {"parts": [{"text": True}]}}]}),
        "text_date": ok({"candidates": [{"content": {"parts": [{"text": "2026-11-17T10:00:00Z"}]}}]}),
        "text_null": ok({"candidates": [{"content": {"parts": [{"text": None}]}}]}),
        "text_missing": ok({"candidates": [{"content": {"parts": [{}]}}]}),
        "text_object": ok({"candidates": [{"content": {"parts": [{"text": {}}]}}]}),
        "parts_string": ok({"candidates": [{"content": {"parts": "x"}}]}),
        "content_array": ok({"candidates": [{"content": []}]}),
        "malformed": ok("{"),
        "root_null": ok("null"),
    },
    "OpenRouter": {
        "http_401": status(401, "Unauthorized", '{"error":{"message":"No auth credentials found"}}'),
        "choices_empty": ok({"choices": []}),
        "content_null": ok({"choices": [{"message": {"content": None}}]}),
        "content_date": ok({"choices": [{"message": {"content": "2026-11-17T10:00:00+02:00"}}]}),
        "html_page": ok("<html>busy</html>", "text/html"),
    },
}
for provider, answers in dynamic_answers.items():
    for name, response in answers.items():
        analyze(f"{provider}_answer_{name}", opts(provider), response, data_file="opaque.png")


def service(kind, id, options, response=None, **extra):
    case = {"id": id, "kind": kind, "options": options, "responses": [response] if response else []}
    case.update(extra)
    cases.append(case)


# Test connection.
long_body = '{"error":"' + "x" * 300 + '"}'
for provider in ["OpenAI", "OpenAILegacy", "Gemini", "OpenRouter"]:
    service("test", f"test_{provider}_ok", opts(provider), ok({"data": []}))
    service("test", f"test_{provider}_401_long", opts(provider), status(401, "Unauthorized", long_body))
service("test", "test_OpenRouter_403_empty", opts("OpenRouter"), status(403, "Forbidden"))
service("test", "test_status_599_no_reason", opts(), status(599, "", "x"))
service("test", "test_OpenAI_missing_key", opts(OpenAIAPIKey="  "))
service("test", "test_Gemini_missing_key", opts("Gemini", GeminiAPIKey=None))
service("test", "test_OpenRouter_missing_key", opts("OpenRouter", OpenRouterAPIKey=""))
service("test", "test_unknown_provider", opts(7))
service("test", "test_Gemini_key_escaped", opts("Gemini", GeminiAPIKey=" a b/c+d?é "), ok({}))
service("test", "test_OpenAI_key_trimmed", opts(OpenAIAPIKey="  sk-padded  "), ok({}))
service("test", "test_OpenAI_custom_url", opts(OpenAICustomURL="http://localhost:8080/"), ok({}))
service("test", "test_OpenAI_custom_url_blank", opts(OpenAICustomURL="   "), ok({}))
service("test", "test_unicode_body", opts(), status(400, "Bad Request", "é" * 250))

# Load models.
service("models", "models_ok_sorted", opts(), ok({"object": "list", "data": [
    {"id": "gpt-5-mini"}, {"id": "GPT-4o"}, {"id": "babbage-002"}, {"id": " "}, {"id": None}, {"id": "gpt-4o"}, {"id": "Ada"}]}))
service("models", "models_legacy", opts("OpenAILegacy"), ok({"data": [{"id": "b"}, {"id": "a"}]}))
service("models", "models_gemini_none", opts("Gemini"), ok({"data": [{"id": "x"}]}))
service("models", "models_missing_data", opts(), ok({"models": []}))
service("models", "models_data_object", opts(), ok({"data": {}}))
service("models", "models_id_number", opts(), ok({"data": [{"id": 1}]}))
service("models", "models_item_missing_id", opts(), ok({"data": [{"name": "x"}]}))
service("models", "models_root_array", opts(), ok([]))
service("models", "models_401", opts(), status(401, "Unauthorized", "{}"))
service("models", "models_empty_body", opts(), ok(""))
service("models", "models_missing_key", opts(OpenAIAPIKey=None))

# ShareX leaves the OpenAI key on its shared HttpClient after an analysis; the
# next request through that client (here Gemini's models list) carries it.
analyze("leak_1_openai_analyze", opts(), OPENAI_OK, data_file="opaque.png")
service("test", "leak_2_gemini_after_openai", opts("Gemini"), ok({}), keep_shared_auth=True,
        expect_differ="ShareX sends the OpenAI key (Authorization) left on its shared HttpClient to Google; ShareXrs doesn't")

assert len({c["id"] for c in cases}) == len(cases), "duplicate ids"
json.dump({"cases": cases}, open(sys.argv[1] if len(sys.argv) > 1 else "analyze_cases.json", "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print(f"{len(cases)} cases")
