class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.running = false
        this.proc = null

        this.dead_procs = []
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.proc.write(cmd.input)
		} else if (cmd.startLSP) {
            if (this.running) {
                return api.Command.create({error: "LSP already running"})
                return
            }

            this.current_ref = cmd.ref
            this.running = true

            const cmd = msg.args[0];
            const args = msg.args.slice(1)

            
            this.proc = new Process(this.id, cmd, args, {})
            
            await this.proc.init(this.clients)
            await this.send(api.Command.create({ok: {}, ref: cmd.ref}), 0)

        } else {
            console.debug("Unknown LSP msg", cmd)
            
        }
	}

    async process_dead(proc_id, exit_code) {
        if (this.dead_procs.includes(proc_id) && proc_id !== -1) {return}
        this.dead_procs.push(proc_id)
        this.running = false

        if (proc_id !== -1) {
            try {
                await this.proc.destroy()
            } catch (_) {}
        }

        await this.send(api.Command.create({state: api.State.Stopped}), 0)
    }

    async attach(session) {
        if (this.proc) await this.proc.add_session(session)
    }

    async detach(session) {
        if (this.proc) await this.proc.remove_session(session)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
