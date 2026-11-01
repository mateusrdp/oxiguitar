#!/usr/bin/env python3
"""Phase 7 — FINALIZE: Assemble final knowledge-graph.json"""

import json
import sys
from datetime import datetime, timezone

def main():
    # Load all intermediate artifacts
    with open("assembled-graph.json") as f:
        graph = json.load(f)
    with open("layers.json") as f:
        layers = json.load(f)
    with open("tour.json") as f:
        tour = json.load(f)
    with open("review-results.json") as f:
        review = json.load(f)
    with open("batches.json") as f:
        batches = json.load(f)

    # Compute stats
    node_count = len(graph["nodes"])
    edge_count = len(graph["edges"])

    # Build final knowledge graph
    knowledge_graph = {
        "meta": {
            "title": "OxiGuitar Knowledge Graph",
            "version": "1.0.0",
            "generatedAt": datetime.now(timezone.utc).isoformat(),
            "project": "OxiGuitar",
            "projectVersion": "0.10.0",
            "commitHash": "520ed911d547fe42021fe8a5475378aaf4da76a7",
            "description": "Complete knowledge graph of the OxiGuitar codebase — a cross-platform guitar tablature editor written in Rust.",
            "stats": {
                "totalNodes": node_count,
                "totalEdges": edge_count,
                "totalLayers": len(layers),
                "totalTourStops": len(tour.get("tourStops", [])),
                "filesScanned": len(batches) if isinstance(batches, list) else len(batches.get("batches", []))
            },
            "review": {
                "grade": review.get("grade", "N/A"),
                "failCount": review.get("fail_count", 0),
                "warnCount": review.get("warn_count", 0)
            }
        },
        "layers": layers,
        "tour": tour,
        "nodes": graph["nodes"],
        "edges": graph["edges"]
    }

    # Write final output
    output_path = "../knowledge-graph.json"
    with open(output_path, "w") as f:
        json.dump(knowledge_graph, f, indent=2)

    print(f"Final knowledge-graph.json written: {output_path}")
    print(f"  Nodes: {node_count}")
    print(f"  Edges: {edge_count}")
    print(f"  Layers: {len(layers)}")
    print(f"  Tour stops: {len(tour.get('tourStops', []))}")
    print(f"  Review grade: {review.get('grade', 'N/A')}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
