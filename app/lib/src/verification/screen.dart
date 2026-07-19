import 'package:flutter/material.dart';

class VerificationScreen extends StatelessWidget {
  const VerificationScreen({
    required this.code,
    required this.peerName,
    required this.onConfirmed,
    required this.onCancelled,
    super.key,
  }) : assert(code.length == 6);

  final String code;
  final String peerName;
  final VoidCallback onConfirmed;
  final VoidCallback onCancelled;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        title: Text(peerName),
      ),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 560),
              child: Column(
                children: [
                  Text(
                    '相手の画面に同じ番号が\n表示されていることを確認してください',
                    style: Theme.of(context).textTheme.titleLarge,
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 40),
                  Semantics(
                    label: code,
                    child: ExcludeSemantics(
                      child: Text(
                        code.split('').join('  '),
                        style: Theme.of(context).textTheme.displayMedium
                            ?.copyWith(
                              fontFeatures: const [
                                FontFeature.tabularFigures(),
                              ],
                              fontWeight: FontWeight.w600,
                              letterSpacing: 2,
                            ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 48),
                  Wrap(
                    alignment: WrapAlignment.center,
                    spacing: 12,
                    runSpacing: 12,
                    children: [
                      FilledButton(
                        onPressed: onConfirmed,
                        child: const Text('一致している'),
                      ),
                      OutlinedButton(
                        onPressed: onCancelled,
                        child: const Text('キャンセル'),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
