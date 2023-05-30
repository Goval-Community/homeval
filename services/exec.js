class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.running = false
        this.proc = null
        this.queue = []

        this.dead_procs = []
    }
    
	async recv(cmd, session) {
		if (cmd.input) {
			await this.proc.write(cmd.input)
		} else if (cmd.exec) {
            if (cmd.exec.args.length === 3) {
                let arg = cmd.exec.args[2]

                if (arg === "date '+%s%N' && cat /sys/fs/cgroup/cpu/cpuacct.usage /sys/fs/cgroup/cpu/cpu.cfs_quota_us /sys/fs/cgroup/cpu/cpu.cfs_period_us /sys/fs/cgroup/memory/memory.usage_in_bytes /sys/fs/cgroup/memory/memory.soft_limit_in_bytes /sys/fs/cgroup/memory/memory.limit_in_bytes &&grep '^\\(total_rss\\|total_cache\\) ' /sys/fs/cgroup/memory/memory.stat" || arg === "cat /repl/stats/subvolume_usage_bytes /repl/stats/subvolume_total_bytes") {
                    return
                }
            }
            if (this.running) {
                if (!blocking) {
                    return api.Command.create({error: "Already running"})
                }

                let invalid = this.validate_exec(cmd.exec)
                if (invalid) {
                    return invalid
                }

                this.queue.push(cmd.exec)
            }

            await this.start_proc(cmd.exec)

            return api.Command.create({ok: {}})
        } else {
            console.debug("Unknown exec cmd", cmd)
        }
	}

    async process_dead(proc_id) {
        if (this.dead_procs.includes(proc_id)) {return}
        this.dead_procs.push(proc_id)
        
        try {
            await this.proc.destroy()
        } catch (_) {}

        await Deno.core.ops.op_sleep(100);
        await this.send(api.Command.create({state: api.State.Stopped}), 0)
        
        if (this.queue.length === 0) {
            return
        }
        
        await this.start_proc(this.queue.shift())
    }

    async validate_exec(cmd) {
        if (cmd.args.length === 0) {
            return api.Command.create({error: "Missing command"})
        }
    }

    async start_proc(msg) {
        const cmd = msg.args[0]
        const args = msg.args.slice(1)
        const env = msg.env ? msg.env : {}

        this.proc = new Process(this.id, cmd, args, env)
        await this.proc.init(this.clients)

        await this.send(api.Command.create({state: api.State.Running}), 0)
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
