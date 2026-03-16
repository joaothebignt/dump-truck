<script setup>

import { reactive } from "vue"
import { useRouter } from "vue-router"

import { useConnectionsStore } from "../stores/connections"

const router = useRouter()
const store = useConnectionsStore()

const form = reactive({

  name: "",
  host: "127.0.0.1", // pode ser IP ou hostname na rede local
  port: 3306,
  username: "",
  password: ""

})

function save() {

  store.add({ ...form })

  router.push("/")

}

</script>

<template>

<h2>Nova conexão</h2>

<input v-model="form.name" placeholder="Nome da conexão" />
<input v-model="form.host" placeholder="Host (ex: 192.168.0.10)" />
<input v-model="form.port" placeholder="Port" />

<input v-model="form.username" placeholder="Username" />

<input
type="password"
v-model="form.password"
placeholder="Password"
/>

<button @click="save">
Salvar
</button>

</template>