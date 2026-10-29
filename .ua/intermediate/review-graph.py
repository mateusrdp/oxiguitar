#!/usr/bin/env python3
"""Phase 6 — REVIEW: Deterministic validation of assembled-graph.json"""

import json
import sys
from collections import Counter

GRAPH_PATH = "assembled-graph.json"
LAYERS_PATH = "layers.json"
OUTPUT_PATH = "review-results.json"

def main():
    with open(GRAPH_PATH) as f:
        graph = json.load(f)
    with open(LAYERS_PATH) as f:
        layers = json.load(f)

    nodes = graph.get("nodes", [])
    edges = graph.get("edges", [])

    results = {
        "total_nodes": len(nodes),
        "total_edges": len(edges),
        "checks": []
    }

    # 1. Duplicate node IDs
    node_ids = [n["id"] for n in nodes]
    dup_ids = [id_ for id_, count in Counter(node_ids).items() if count > 1]
    dup_check = {
        "name": "duplicate_node_ids",
        "status": "fail" if dup_ids else "pass",
        "details": f"{len(dup_ids)} duplicate node IDs found" if dup_ids else "No duplicate node IDs"
    }
    if dup_ids:
        dup_check["duplicates"] = dup_ids[:20]
    results["checks"].append(dup_check)

    # 2. Dangling edges (source or target not in node set)
    node_set = set(node_ids)
    dangling = []
    for i, e in enumerate(edges):
        src = e.get("source")
        tgt = e.get("target")
        if src and src not in node_set:
            dangling.append({"edge_idx": i, "field": "source", "ref": src})
        if tgt and tgt not in node_set:
            dangling.append({"edge_idx": i, "field": "target", "ref": tgt})
    dangling_check = {
        "name": "dangling_edges",
        "status": "fail" if dangling else "pass",
        "details": f"{len(dangling)} dangling edge references" if dangling else "No dangling edges"
    }
    if dangling:
        dangling_check["examples"] = dangling[:10]
    results["checks"].append(dangling_check)

    # 3. Missing 'type' field on edges
    missing_edge_type = [i for i, e in enumerate(edges) if "type" not in e or not e["type"]]
    edge_type_check = {
        "name": "edge_type_field",
        "status": "warn" if missing_edge_type else "pass",
        "details": f"{len(missing_edge_type)} edges missing 'type' field"
    }
    results["checks"].append(edge_type_check)

    # 4. Missing 'type' field on nodes
    missing_node_type = [n["id"] for n in nodes if "type" not in n or not n["type"]]
    node_type_check = {
        "name": "node_type_field",
        "status": "warn" if missing_node_type else "pass",
        "details": f"{len(missing_node_type)} nodes missing 'type' field"
    }
    results["checks"].append(node_type_check)

    # 5. Missing 'category' field on nodes
    missing_category = [n["id"] for n in nodes if "category" not in n or not n["category"]]
    cat_dist = Counter(n.get("category", "none") for n in nodes)
    cat_check = {
        "name": "node_category_field",
        "status": "info",
        "details": f"{len(missing_category)} nodes missing 'category' field",
        "category_distribution": dict(sorted(cat_dist.items(), key=lambda x: -x[1]))
    }
    results["checks"].append(cat_check)

    # 6. Layer coverage
    covered_ids = set()
    for layer in layers:
        for nid in layer.get("nodeIds", []):
            covered_ids.add(nid)
    covered_in_graph = covered_ids & node_set
    not_in_graph = covered_ids - node_set
    graph_not_labeled = node_set - covered_ids
    layer_check = {
        "name": "layer_coverage",
        "status": "pass" if not not_in_graph else "warn",
        "details": f"{len(covered_in_graph)} nodes covered by layers, {len(not_in_graph)} layer nodes not in graph, {len(graph_not_labeled)} graph nodes not in any layer",
        "layers_count": len(layers),
        "not_in_graph_examples": list(not_in_graph)[:10]
    }
    results["checks"].append(layer_check)

    # 7. Edge type distribution
    edge_types = Counter(e.get("type", "missing") for e in edges)
    results["checks"].append({
        "name": "edge_type_distribution",
        "status": "info",
        "distribution": dict(sorted(edge_types.items(), key=lambda x: -x[1]))
    })

    # 8. Node type distribution
    node_types = Counter(n.get("type", "unknown") for n in nodes)
    results["checks"].append({
        "name": "node_type_distribution",
        "status": "info",
        "distribution": dict(list(sorted(node_types.items(), key=lambda x: -x[1]))[:20])
    })

    # 9. Overall grade
    fails = [c for c in results["checks"] if c["status"] == "fail"]
    warns = [c for c in results["checks"] if c["status"] == "warn"]
    if not fails and not warns:
        grade = "A"
    elif not fails:
        grade = "B"
    elif len(fails) == 1 and len(warns) <= 3:
        grade = "B+"
    elif len(fails) <= 2:
        grade = "C"
    else:
        grade = "D"
    results["grade"] = grade
    results["fail_count"] = len(fails)
    results["warn_count"] = len(warns)

    with open(OUTPUT_PATH, "w") as f:
        json.dump(results, f, indent=2)

    print(f"Review complete: Grade {grade}")
    print(f"  Fails: {len(fails)}, Warnings: {len(warns)}")
    for c in results["checks"]:
        detail = c.get("details", "")
        print(f"  [{c['status'].upper()}] {c['name']}: {detail}")
    return 0 if not fails else 1

if __name__ == "__main__":
    sys.exit(main())
