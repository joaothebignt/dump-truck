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
        password: connection.password
      })

      this.save()
    },

    remove(index) {

      this.favorites.splice(index, 1)
      this.save()

    }

  }

})