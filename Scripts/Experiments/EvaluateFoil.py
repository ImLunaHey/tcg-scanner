#!/usr/bin/env python3
"""Offline development analysis; never called by ScannerService.

Usage:
  python3 Scripts/Experiments/EvaluateFoil.py --self-test
  python3 Scripts/Experiments/EvaluateFoil.py measurements.json groups.json output.json
  python3 Scripts/Experiments/EvaluateFoil.py --apply fitted.json measurements.json output.json
  python3 Scripts/Experiments/EvaluateFoil.py --predict fitted.json measurements.json output.json

Measurements: [{"input": {"id": "B-025/C-001", "label":
"user-reported-non-holo", "printingIDs": [], "path": "/prepared.png"},
"variant": "baseline", "features": {"borderChroma": 0.02}}].
Groups: {"B-025/C-001": {"group": "claydol", "split": "calibration"}}.

For every feature, fit max(calibration-negative baseline values) * 1.25 +
0.001. Apply that SAME threshold to every variant and report all outcomes.
Also leave each whole name group out, fitting other baseline negatives only.
The reused holdout and leave-group-out results are exploratory, not independent
validation. Rank only by calibration baseline positives detected, then name.
Reads JSON measurements only: no image, network, scanner, database or label
mutations. Output must be a new file, separate from both inputs.

--apply uses saved thresholds without fitting and accepts single-class datasets.
All cards must have the same complete variant set, including baseline. Stability
is three-way: every variant above threshold => flaggedFoil; none above =>
noFoilEvidence; disagreement => unstable. Neither latter status proves non-holo.

--predict requires label exactly "unlabelled" for every measurement. It uses
saved thresholds and reports per-card consensus only, without physical labels,
accuracy or confusion counts. Aggregate batch finish counts are not an input.
"""

import copy
import json
import math
import pathlib
import sys


POSITIVE = "user-reported-holo"
NEGATIVE = "user-reported-non-holo"
UNLABELLED = "unlabelled"


def validate(measurements, groups, require_calibration_negative=True,
             allowed_labels=(POSITIVE, NEGATIVE)):
    if not isinstance(measurements, list) or not 1 <= len(measurements) <= 10000:
        raise ValueError("Expected 1–10000 measurement rows")
    if not isinstance(groups, dict):
        raise ValueError("Expected a group mapping")
    seen, baselines, rows, splits, card_variants = set(), {}, [], {}, {}
    feature_names = None
    for item in measurements:
        if not isinstance(item, dict) or not isinstance(item.get("input"), dict):
            raise ValueError("Expected a measurement with input metadata")
        ident = item["input"].get("id")
        label = item["input"].get("label")
        variant = item.get("variant")
        if not isinstance(ident, str) or not ident or ident not in groups:
            raise ValueError("Each measurement needs an ID present in groups")
        if label not in allowed_labels:
            raise ValueError("Expected labels: " + ", ".join(allowed_labels))
        if not isinstance(variant, str) or not variant:
            raise ValueError("Each measurement needs a variant")
        if (ident, variant) in seen:
            raise ValueError("Duplicate card/variant")
        seen.add((ident, variant))
        card_variants.setdefault(ident, set()).add(variant)
        grouping = groups[ident]
        if not isinstance(grouping, dict):
            raise ValueError("Invalid grouping metadata")
        group, split = grouping.get("group"), grouping.get("split")
        if not isinstance(group, str) or not group or split not in ("calibration", "holdout"):
            raise ValueError("Group name and calibration/holdout split required")
        if group in splits and splits[group] != split:
            raise ValueError("A name group cannot cross splits")
        splits[group] = split
        features = item.get("features")
        if not isinstance(features, dict) or not features or len(features) > 1000:
            raise ValueError("Expected 1–1000 features per row")
        for name, value in features.items():
            if not isinstance(name, str) or not name or isinstance(value, bool) or not isinstance(value, (int, float)):
                raise ValueError("Expected numeric named features")
            if not math.isfinite(value) or value < 0:
                raise ValueError("Features must be finite and nonnegative")
        if feature_names is None:
            feature_names = set(features)
        if set(features) != feature_names:
            raise ValueError("Every row must have identical feature names")
        row = dict(id=ident, label=label, variant=variant, group=group,
                   split=split, features=features)
        rows.append(row)
        if variant == "baseline":
            baselines[ident] = row
    for row in rows:
        if row["id"] not in baselines:
            raise ValueError("Each card needs a baseline measurement")
        if row["label"] != baselines[row["id"]]["label"]:
            raise ValueError("A card's physical label cannot change between variants")
    expected_variants = set.union(*card_variants.values())
    if any(variants != expected_variants for variants in card_variants.values()):
        raise ValueError("Every card must have the same complete variant set")
    if require_calibration_negative and not any(r["label"] == NEGATIVE and r["split"] == "calibration" for r in baselines.values()):
        raise ValueError("At least one calibration negative baseline is required")
    return rows, sorted(feature_names)


def threshold_for(rows, feature):
    negatives = [r["features"][feature] for r in rows if r["label"] == NEGATIVE]
    if not negatives:
        return None
    threshold = max(negatives) * 1.25 + 0.001
    if not math.isfinite(threshold):
        raise ValueError("Feature magnitude overflows threshold calculation")
    return threshold


def outcomes(rows, feature, threshold):
    counts = dict(TP=0, FN=0, FP=0, TN=0)
    predictions = []
    for row in sorted(rows, key=lambda r: r["id"]):
        flagged = row["features"][feature] > threshold
        outcome = ("T" if flagged == (row["label"] == POSITIVE) else "F") + ("P" if flagged else "N")
        counts[outcome] += 1
        predictions.append(dict(id=row["id"], group=row["group"],
                                label=row["label"], value=row["features"][feature],
                                flaggedFoil=flagged, outcome=outcome))
    return dict(counts=counts, predictions=predictions)


def evaluate(measurements, groups):
    rows, features = validate(measurements, groups)
    baseline = [r for r in rows if r["variant"] == "baseline"]
    calibration = [r for r in baseline if r["split"] == "calibration"]
    variants = sorted({r["variant"] for r in rows})
    results = []
    for feature in features:
        threshold = threshold_for(calibration, feature)
        scores = {
            variant: {
                split: outcomes([r for r in rows if r["variant"] == variant and r["split"] == split], feature, threshold)
                for split in ("calibration", "holdout")
            } for variant in variants
        }
        folds, aggregate, skipped = [], dict(TP=0, FN=0, FP=0, TN=0), []
        for group in sorted({r["group"] for r in baseline}):
            training = [r for r in baseline if r["group"] != group]
            held_out = [r for r in baseline if r["group"] == group]
            fold_threshold = threshold_for(training, feature)
            if fold_threshold is None:
                skipped.append(group)
                continue
            score = outcomes(held_out, feature, fold_threshold)
            for key, value in score["counts"].items():
                aggregate[key] += value
            folds.append(dict(group=group, threshold=fold_threshold,
                              trainingNegativeIDs=sorted(r["id"] for r in training if r["label"] == NEGATIVE), **score))
        results.append(dict(feature=feature, threshold=threshold, variants=scores,
                            leaveNameGroupOut=dict(counts=aggregate, folds=folds, skippedGroups=skipped)))
    results.sort(key=lambda r: (-r["variants"]["baseline"]["calibration"]["counts"]["TP"], r["feature"]))
    return dict(schemaVersion=1, status="exploratory-not-independent-validation",
                thresholdRule="max(calibration-negative baseline) * 1.25 + 0.001; flag strictly greater",
                ranking="calibration baseline TP descending, feature name ascending",
                caveats=["Holdout has already been inspected and is reused; it is not fresh validation.",
                         "Leave-name-group-out also reuses this dataset; it does not remove set or artwork confounding.",
                         "All features are reported; no production finish labels are changed."],
                features=results)


def consensus_predictions(rows, feature, threshold):
    cards = {}
    for row in rows:
        cards.setdefault(row["id"], []).append(row)
    predictions = []
    for ident, variants in sorted(cards.items()):
        flags = [row["features"][feature] > threshold for row in variants]
        status = "flaggedFoil" if all(flags) else "unstable" if any(flags) else "noFoilEvidence"
        predictions.append(dict(id=ident, status=status,
                                values={row["variant"]: row["features"][feature]
                                        for row in sorted(variants, key=lambda r: r["variant"])}))
    return predictions


def stability(rows, feature, threshold):
    labels = {row["id"]: row["label"] for row in rows}
    counts = {label: dict(flaggedFoil=0, noFoilEvidence=0, unstable=0)
              for label in (POSITIVE, NEGATIVE)}
    predictions = consensus_predictions(rows, feature, threshold)
    for prediction in predictions:
        label = labels[prediction["id"]]
        prediction["label"] = label
        counts[label][prediction["status"]] += 1
    return dict(countsByPhysicalLabel=counts, predictions=predictions)


def measurement_groups(measurements):
    if not isinstance(measurements, list):
        raise ValueError("Expected measurement rows")
    groups = {}
    for row in measurements:
        if not isinstance(row, dict) or not isinstance(row.get("input"), dict):
            raise ValueError("Expected a measurement with input metadata")
        ident = row["input"].get("id")
        if not isinstance(ident, str) or not ident:
            raise ValueError("Each measurement needs a nonempty ID")
        groups[ident] = dict(group=ident, split="holdout")
    return groups


def fitted_thresholds(fitted, features):
    if not isinstance(fitted, dict) or fitted.get("schemaVersion") != 1:
        raise ValueError("Expected a schemaVersion 1 fitted evaluation")
    fitted_features = fitted.get("features")
    if not isinstance(fitted_features, list) or not 1 <= len(fitted_features) <= 1000:
        raise ValueError("Expected 1–1000 fitted feature thresholds")
    thresholds = {}
    for item in fitted_features:
        if not isinstance(item, dict):
            raise ValueError("Invalid fitted feature")
        name, threshold = item.get("feature"), item.get("threshold")
        if not isinstance(name, str) or not name or name in thresholds:
            raise ValueError("Fitted feature names must be nonempty and unique")
        if isinstance(threshold, bool) or not isinstance(threshold, (int, float)) or not math.isfinite(threshold) or threshold < 0:
            raise ValueError("Fitted thresholds must be finite and nonnegative")
        thresholds[name] = threshold
    if set(thresholds) != set(features):
        raise ValueError("Measurements and fitted evaluation must have identical feature names")
    return thresholds


def apply_fitted(fitted, measurements):
    rows, features = validate(measurements, measurement_groups(measurements), require_calibration_negative=False)
    thresholds = fitted_thresholds(fitted, features)
    variants = sorted({r["variant"] for r in rows})
    results = []
    for feature in features:
        threshold = thresholds[feature]
        results.append(dict(feature=feature, threshold=threshold,
                            variants={variant: outcomes([r for r in rows if r["variant"] == variant], feature, threshold)
                                      for variant in variants},
                            stability=stability(rows, feature, threshold)))
    return dict(schemaVersion=1, status="exploratory-fixed-threshold-application",
                thresholdRule="Saved fitted thresholds; no fitting or ranking on this dataset",
                caveats=["NoFoilEvidence does not establish non-holo; unstable is not forced negative.",
                         "Dataset/design confounding and prior data inspection still limit validation.",
                         "Physical labels are used only to report results, never to alter thresholds."],
                features=results)


def predict_fitted(fitted, measurements):
    rows, features = validate(measurements, measurement_groups(measurements),
                              require_calibration_negative=False, allowed_labels=(UNLABELLED,))
    thresholds = fitted_thresholds(fitted, features)
    return dict(schemaVersion=1, status="unlabelled-fixed-threshold-predictions",
                thresholdRule="Saved fitted thresholds; no fitting, label inference or batch-count matching",
                caveats=["Predictions only; physical per-card labels are unknown and accuracy is not computed.",
                         "NoFoilEvidence does not establish non-holo; disagreement remains unstable.",
                         "A foil signal does not distinguish holo from reverse holo."],
                features=[dict(feature=feature, threshold=thresholds[feature],
                               predictions=consensus_predictions(rows, feature, thresholds[feature]))
                          for feature in features])


def self_test():
    def row(ident, label, value, variant="baseline"):
        return dict(input=dict(id=ident, label=label), variant=variant, features={"signal": value})
    data = [row("n1", NEGATIVE, .01), row("n2", NEGATIVE, .02),
            row("n3", NEGATIVE, .03), row("h1", POSITIVE, .1), row("h2", POSITIVE, .1)]
    groups = {"n1": dict(group="normal-a", split="calibration"),
              "n2": dict(group="normal-a", split="calibration"),
              "n3": dict(group="normal-b", split="holdout"),
              "h1": dict(group="holo-a", split="calibration"),
              "h2": dict(group="holo-b", split="holdout")}
    result = evaluate(data, groups)["features"][0]
    assert abs(result["threshold"] - .026) < 1e-12
    assert result["variants"]["baseline"]["holdout"]["counts"] == dict(TP=1, FN=0, FP=1, TN=0)
    changed = copy.deepcopy(data)
    changed[2]["features"]["signal"] = 1e6
    assert evaluate(changed, groups)["features"][0]["threshold"] == result["threshold"]
    fold = next(f for f in result["leaveNameGroupOut"]["folds"] if f["group"] == "normal-a")
    assert fold["trainingNegativeIDs"] == ["n3"]
    assert {r["id"] for r in fold["predictions"]} == {"n1", "n2"}
    changed = data + [row(r["input"]["id"], r["input"]["label"], r["features"]["signal"], "inset") for r in data]
    changed[len(data)]["features"]["signal"] = .5
    variant = evaluate(changed, groups)["features"][0]
    assert variant["threshold"] == result["threshold"]
    assert variant["variants"]["inset"]["calibration"]["counts"]["FP"] == 1

    def rejects(rows, grouping=groups):
        try:
            evaluate(rows, grouping)
        except ValueError:
            return
        raise AssertionError("Invalid inputs were accepted")

    rejects(data + [row("n1", POSITIVE, .5, "inset")])
    rejects(data + [row("n1", NEGATIVE, .5, "inset")])
    rejects(data + [row("n1", NEGATIVE, .5)])
    for value in (float("nan"), float("inf"), -1, True):
        changed = copy.deepcopy(data)
        changed[0]["features"]["signal"] = value
        rejects(changed)
    changed = copy.deepcopy(data)
    changed[0]["input"]["label"] = "guessed-holo"
    rejects(changed)
    changed_groups = copy.deepcopy(groups)
    changed_groups["n2"]["split"] = "holdout"
    rejects(data, changed_groups)

    fitted = evaluate(data, groups)
    positive_only = [row("p1", POSITIVE, .1), row("p1", POSITIVE, .01, "inset"),
                     row("p2", POSITIVE, .1), row("p2", POSITIVE, .1, "inset")]
    applied = apply_fitted(fitted, positive_only)["features"][0]
    assert applied["threshold"] == result["threshold"]
    assert applied["stability"]["countsByPhysicalLabel"][POSITIVE] == dict(flaggedFoil=1, noFoilEvidence=0, unstable=1)
    assert applied["stability"]["predictions"][0]["status"] == "unstable"
    negative_only = [row("n", NEGATIVE, .01), row("n", NEGATIVE, .01, "inset")]
    assert apply_fitted(fitted, negative_only)["features"][0]["stability"]["countsByPhysicalLabel"][NEGATIVE]["noFoilEvidence"] == 1
    negative_only[0]["features"]["signal"] = 1e6
    assert apply_fitted(fitted, negative_only)["features"][0]["threshold"] == result["threshold"]
    try:
        apply_fitted(fitted, positive_only[:-1])
    except ValueError:
        pass
    else:
        raise AssertionError("Apply accepted incomplete variants")
    blind = [row("b1", UNLABELLED, .1), row("b1", UNLABELLED, .01, "inset"),
             row("b2", UNLABELLED, .1), row("b2", UNLABELLED, .1, "inset"),
             row("b3", UNLABELLED, .01), row("b3", UNLABELLED, .01, "inset")]
    predicted = predict_fitted(fitted, blind)
    feature = predicted["features"][0]
    assert feature["threshold"] == result["threshold"]
    assert [p["status"] for p in feature["predictions"]] == ["unstable", "flaggedFoil", "noFoilEvidence"]
    assert set(feature) == {"feature", "threshold", "predictions"}
    assert all(set(p) == {"id", "status", "values"} for p in feature["predictions"])
    changed_blind = copy.deepcopy(blind)
    changed_blind[0]["features"]["signal"] = 1e6
    assert predict_fitted(fitted, changed_blind)["features"][0]["threshold"] == feature["threshold"]

    def rejects_prediction(rows):
        try:
            predict_fitted(fitted, rows)
        except ValueError:
            return
        raise AssertionError("Invalid blind inputs were accepted")

    rejects_prediction(blind[:-1])
    for label in (POSITIVE, NEGATIVE, None, "unknown", ""):
        changed_blind = copy.deepcopy(blind)
        changed_blind[0]["input"]["label"] = label
        rejects_prediction(changed_blind)
    for labelled_mode in (lambda: evaluate(blind, measurement_groups(blind)),
                          lambda: apply_fitted(fitted, blind)):
        try:
            labelled_mode()
        except ValueError:
            pass
        else:
            raise AssertionError("Labelled mode accepted unlabelled data")
    print("Evaluator checks passed: threshold isolation, grouped folds, complete variants, fixed-threshold application and strict unlabelled consensus.")


def main(argv):
    if argv == ["--self-test"]:
        self_test()
        return
    applying = len(argv) == 4 and argv[0] == "--apply"
    predicting = len(argv) == 4 and argv[0] == "--predict"
    if len(argv) != 3 and not applying and not predicting:
        raise ValueError("Usage: EvaluateFoil.py measurements.json groups.json output.json | --apply fitted.json measurements.json output.json | --predict fitted.json measurements.json output.json | --self-test")
    first_path, second_path, output_path = map(pathlib.Path, argv[1:] if applying or predicting else argv)
    if output_path.resolve() in (first_path.resolve(), second_path.resolve()):
        raise ValueError("Output must be separate from inputs")
    first, second = json.loads(first_path.read_text()), json.loads(second_path.read_text())
    result = predict_fitted(first, second) if predicting else apply_fitted(first, second) if applying else evaluate(first, second)
    encoded = json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n"
    with output_path.open("x") as output:
        output.write(encoded)
    print(f"Evaluated {len(result['features'])} features; exploratory results only; finish labels unchanged.")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except (ValueError, OSError, TypeError) as error:
        print(f"EvaluateFoil: {error}", file=sys.stderr)
        sys.exit(1)
