class Service extends ServiceBase {
	async recv(cmd, session) {
		if (cmd.readdir) {
			let files = []
			
			try {
				files = await fs.readDir(cmd.readdir.path)
			} catch(err) {
				return api.Command.create({error: err.toString()})
			}

			return api.Command.create({
				files: { files: files.map(item => {return {path: item.path, type: item.type !== "directory" ? api.File.Type.FILE : api.File.Type.DIRECTORY}}) },
			});
		} 
		if (cmd.write) {
			let contents = cmd.write.content
			if (contents.length === 0) {
				contents = []
			}
			await fs.writeFile(cmd.write.path, contents)
			return api.Command.create({ok:{}})
		}
		if (cmd.read) {
			let contents;
			if (cmd.read.path === ".config/goval/info") {
				const encoder = new TextEncoder();
				contents = encoder.encode(JSON.stringify({
					"server": "homeval",
					"version": "1.0.0a", // TODO: real thing
					"author": "PotentialStyx",
					"uptime": 0, // seconds, TODO: real thing
					"services": Deno.core.ops.op_get_supported_services()
				}))
			} else {
				contents = await fs.readFile(cmd.read.path);
			}

			return api.Command.create({file:{path:cmd.read.path, content: contents}})
		}
		if (cmd.remove) {
			await fs.remove(cmd.remove.path)
			return api.Command.create({ok:{}})
		}
		if (cmd.move) {
			await fs.rename(cmd.move.oldPath, cmd.move.newPath)
			return api.Command.create({ok:{}})
		}
	}
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()