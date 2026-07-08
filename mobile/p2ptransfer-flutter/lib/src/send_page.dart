import 'package:flutter/material.dart';
import 'package:file_picker/file_picker.dart';

class SendPage extends StatefulWidget {
  const SendPage({super.key});

  @override
  State<SendPage> createState() => _SendPageState();
}

class _SendPageState extends State<SendPage> {
  String? _filePath;
  final _peerController = TextEditingController(text: '192.168.1.100:9877');
  double _compression = 10;

  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles();
    if (result != null) {
      setState(() => _filePath = result.files.single.path);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Send File')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('File:', style: TextStyle(fontSize: 16)),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: Text(_filePath ?? 'No file selected',
                      style: const TextStyle(fontFamily: 'monospace')),
                ),
                ElevatedButton(onPressed: _pickFile, child: const Text('Browse')),
              ],
            ),
            const SizedBox(height: 20),
            const Text('Peer Address:', style: TextStyle(fontSize: 16)),
            const SizedBox(height: 8),
            TextField(controller: _peerController, decoration: const InputDecoration(border: OutlineInputBorder())),
            const SizedBox(height: 20),
            const Text('Compression Level:', style: TextStyle(fontSize: 16)),
            Slider(value: _compression, min: 1, max: 22, divisions: 21,
                label: _compression.round().toString(),
                onChanged: (v) => setState(() => _compression = v)),
            const SizedBox(height: 32),
            SizedBox(
              width: double.infinity,
              child: ElevatedButton.icon(
                onPressed: _filePath == null ? null : () {},
                icon: const Icon(Icons.send),
                label: const Text('Send'),
                style: ElevatedButton.styleFrom(padding: const EdgeInsets.all(16)),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
