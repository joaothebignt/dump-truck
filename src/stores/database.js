import { defineStore } from "pinia"

export const useDatabaseStore = defineStore("database", {

  state: () => ({

    connection: null,

    databases: [],

    selectedDatabase: null,

    tables: []

  }),

  actions: {

    setConnection(conn){

      this.connection = conn

    },

    clearConnection(){

      this.connection = null
      this.databases = []
      this.selectedDatabase = null
      this.tables = []

    }

  }

})