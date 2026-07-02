package com.p2p.core.util;

import java.util.ArrayList;
import java.util.List;

/**
 * Minimal streaming JSON array parser — no dependencies.
 * Extracts individual JSON objects from a JSON array string.
 */
public final class JsonArrayParser {

    private final String json;
    private int pos;

    public JsonArrayParser(String json) {
        this.json = json.trim();
        this.pos = 0;
    }

    public boolean hasNext() {
        skipWhitespace();
        if (pos >= json.length()) return false;
        if (json.charAt(pos) == ']') return false;
        if (json.charAt(pos) == ',') { pos++; skipWhitespace(); }
        return pos < json.length() && json.charAt(pos) == '{';
    }

    public String nextObject() {
        if (!hasNext()) return null;
        int start = pos;
        int depth = 0;
        boolean inStr = false;
        while (pos < json.length()) {
            char c = json.charAt(pos);
            if (inStr) {
                if (c == '\\') pos++;
                else if (c == '"') inStr = false;
            } else {
                if (c == '"') inStr = true;
                else if (c == '{') depth++;
                else if (c == '}') { depth--; if (depth == 0) { pos++; break; } }
            }
            pos++;
        }
        return json.substring(start, pos);
    }

    private void skipWhitespace() {
        while (pos < json.length() && Character.isWhitespace(json.charAt(pos))) pos++;
    }

    public static List<String> parseArray(String json) {
        var list = new ArrayList<String>();
        var parser = new JsonArrayParser(json);
        while (parser.hasNext()) {
            String obj = parser.nextObject();
            if (obj != null) list.add(obj);
        }
        return list;
    }
}

