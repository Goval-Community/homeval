class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        Deno.core.ops.op_register_pty(["zsh"], this.id).then(pty_id => {
            console.log("GOT PTY ID:", pty_id)
            this.pty = pty_id
        })
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await Deno.core.ops.op_pty_write_msg(this.pty, cmd.input)
		}

        // console.log(cmd, this)
	}

    async attach(session) {
        await Deno.core.ops.op_pty_add_session(this.pty, session)
    }
}

console.log(serviceInfo);
const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service._recv();
