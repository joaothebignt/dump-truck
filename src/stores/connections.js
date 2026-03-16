import { defineStore } from "pinia"

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

    }

  }

})