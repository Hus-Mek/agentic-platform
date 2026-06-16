#!/usr/bin/env python3
"""Quick graph query helper for OpenCode agents.
Usage: python graphify_query.py "your question about the codebase"
"""
import sys
import json
from pathlib import Path

# Add graphify to path
sys.path.insert(0, str(Path(__file__).parent))

from graphify.build import build_from_json
from graphify.cache import load_graph

def query_graph(question: str, graph_path: str = "graphify-out/graph.json"):
    """Query the graph and return relevant context."""
    G = load_graph(Path(graph_path))
    
    # Simple keyword matching against node labels
    words = question.lower().split()
    matches = []
    for node_id, data in G.nodes(data=True):
        label = data.get('label', '').lower()
        if any(w in label for w in words if len(w) > 2):
            matches.append({
                'id': node_id,
                'label': data.get('label', ''),
                'type': data.get('type', ''),
                'community': data.get('community', ''),
            })
    
    # Return top matches
    return matches[:10]

if __name__ == "__main__":
    question = " ".join(sys.argv[1:])
    results = query_graph(question)
    print(json.dumps(results, indent=2))
