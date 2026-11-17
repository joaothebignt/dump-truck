import { defineStore } from "pinia"

export function normalizeConnection(connection) {
  if (!connection || typeof connection !== "object")
    return null

  const host = String(connection.host || "").trim()
  if (!host)
    return null

  return {
    name: String(connection.name || "").trim(),
    host,
    port: Number(connection.port) || 3306,
    username: String(connection.username || ""),
    password: String(connection.password || ""),
    color: connection.color || "#3a7afe"
  }
}

export function connectionsFromImport(data) {
  if (Array.isArray(data))
    return data

  if (data && typeof data === "object") {
    if (Array.isArray(data.connections))
      return data.connections
    if (Array.isArray(data.favorites))
      return data.favorites
  }

  return null
}

export const useConnectionsStore = defineStore("connections", {

  state: () => ({
    favorites: []
  }),

  actions: {

    load() {

      const data = localStorage.getItem("connections")

      if (data)
        this.favorites = JSON.parse(data)

    },

    save() {

      localStorage.setItem(
        "connections",
        JSON.stringify(this.favorites)
      )

    },

    add(connection) {

      this.favorites.push({
        name: connection.name,
        host: connection.host,
        port: Number(connection.port),
        username: connection.username,
        password: connection.password,
        color: connection.color || "#3a7afe"
      })

      this.save()
    },

    update(index, connection) {

      if (index < 0 || index >= this.favorites.length)
        return

      this.favorites[index] = {
        ...this.favorites[index],
        name: connection.name,
        host: connection.host,
        port: Number(connection.port),
        username: connection.username,
        password: connection.password,
        color: connection.color || this.favorites[index].color || "#3a7afe"
      }

      this.save()
    },

    remove(index) {

      this.favorites.splice(index, 1)
      this.save()

    },

    exportPayload() {

      return {
        type: "dumper-connections",
        version: 1,
        connections: this.favorites
          .map(normalizeConnection)
          .filter(Boolean)
      }

    },

    applyImport(list, { replace = false } = {}) {

      const normalized = (list || [])
        .map(normalizeConnection)
        .filter(Boolean)

      if (replace)
        this.favorites = normalized
      else
        this.favorites.push(...normalized)

      this.save()
      return normalized.length

    }

  }

})