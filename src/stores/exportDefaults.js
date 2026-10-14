import { defineStore } from "pinia"

const STORAGE_KEY = "exportDefaults"

const DEFAULT_RULES = [
  {
    type: "suffix",
    value: "log",
    excludeStructure: true,
    excludeContent: true,
    excludeDrop: true
  },
  {
    type: "prefix",
    value: "bi_",
    excludeStructure: true,
    excludeContent: true,
    excludeDrop: true
  }
]

function normalizeRule(rule) {
  const value = String(rule?.value || "").trim()
  if (!value)
    return null

  return {
    type: rule?.type === "prefix" ? "prefix" : "suffix",
    value,
    excludeStructure: rule?.excludeStructure !== false,
    excludeContent: rule?.excludeContent !== false,
    excludeDrop: rule?.excludeDrop !== false
  }
}

function loadState() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) {
      return {
        rules: DEFAULT_RULES.map(r => ({ ...r })),
        lastSaveDir: ""
      }
    }

    const parsed = JSON.parse(raw)
    const rules = Array.isArray(parsed.rules)
      ? parsed.rules.map(normalizeRule).filter(Boolean)
      : []

    return {
      rules: rules.length ? rules : DEFAULT_RULES.map(r => ({ ...r })),
      lastSaveDir: typeof parsed.lastSaveDir === "string" ? parsed.lastSaveDir : ""
    }
  } catch {
    return {
      rules: DEFAULT_RULES.map(r => ({ ...r })),
      lastSaveDir: ""
    }
  }
}

export function tableMatchesExportRule(tableName, rule) {
  if (!tableName || !rule?.value)
    return false

  const name = String(tableName).toLowerCase()
  const value = String(rule.value).toLowerCase()

  if (rule.type === "prefix")
    return name.startsWith(value)

  return name.endsWith(value)
}

export function selectionFromExportDefaults(tableName, rules) {
  let structure = true
  let content = true
  let drop = true

  for (const rule of rules || []) {
    if (!tableMatchesExportRule(tableName, rule))
      continue

    if (rule.excludeStructure)
      structure = false
    if (rule.excludeContent)
      content = false
    if (rule.excludeDrop)
      drop = false
  }

  return { structure, content, drop }
}

/** @deprecated use selectionFromExportDefaults */
export function isExcludedByExportDefaults(tableName, rules) {
  const sel = selectionFromExportDefaults(tableName, rules)
  return !sel.structure && !sel.content && !sel.drop
}

export const useExportDefaultsStore = defineStore("exportDefaults", {

  state: () => loadState(),

  actions: {

    persist() {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({
          rules: this.rules,
          lastSaveDir: this.lastSaveDir
        })
      )
    },

    setRules(rules) {
      this.rules = (rules || []).map(normalizeRule).filter(Boolean)
      this.persist()
    },

    addRule(rule) {
      const normalized = normalizeRule(rule)
      if (!normalized)
        return

      this.rules.push(normalized)
      this.persist()
    },

    removeRule(index) {
      if (index < 0 || index >= this.rules.length)
        return

      this.rules.splice(index, 1)
      this.persist()
    },

    setLastSaveDir(dir) {
      this.lastSaveDir = dir || ""
      this.persist()
    }

  }

})
