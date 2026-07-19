import 'package:airtoss/src/model/peer.dart';
import 'package:flutter/material.dart';

class DiscoveryScreen extends StatelessWidget {
  const DiscoveryScreen({
    required this.peers,
    this.isScanning = false,
    this.failureMessage,
    this.onPeerSelected,
    this.onRetry,
    super.key,
  });

  final List<PeerView> peers;
  final bool isScanning;
  final String? failureMessage;
  final ValueChanged<PeerView>? onPeerSelected;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        title: const Text('AirToss'),
      ),
      body: SafeArea(
        child: Align(
          alignment: Alignment.topCenter,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 720),
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    '近くの端末',
                    style: Theme.of(context).textTheme.headlineMedium,
                  ),
                  const SizedBox(height: 8),
                  _Status(
                    isScanning: isScanning,
                    failureMessage: failureMessage,
                    onRetry: onRetry,
                  ),
                  const SizedBox(height: 24),
                  Expanded(
                    child: peers.isEmpty
                        ? const _EmptyState()
                        : ListView.separated(
                            itemCount: peers.length,
                            separatorBuilder: (context, index) =>
                                const SizedBox(height: 12),
                            itemBuilder: (context, index) {
                              final peer = peers[index];
                              return _PeerCard(
                                peer: peer,
                                onTap: onPeerSelected == null
                                    ? null
                                    : () => onPeerSelected!(peer),
                              );
                            },
                          ),
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

class _Status extends StatelessWidget {
  const _Status({
    required this.isScanning,
    required this.failureMessage,
    required this.onRetry,
  });

  final bool isScanning;
  final String? failureMessage;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    if (failureMessage != null) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            Icons.bluetooth_disabled,
            color: Theme.of(context).colorScheme.error,
          ),
          const SizedBox(width: 10),
          Expanded(child: Text(failureMessage!)),
          if (onRetry != null)
            TextButton(onPressed: onRetry, child: const Text('再試行')),
        ],
      );
    }

    if (isScanning) {
      return const Row(
        children: [
          SizedBox.square(
            dimension: 18,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          SizedBox(width: 10),
          Text('Bluetooth で探しています'),
        ],
      );
    }

    return const Text('Bluetooth での探索は停止しています');
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.only(bottom: 72),
        child: Text(
          '相手の端末でも AirToss を開いてください',
          style: Theme.of(context).textTheme.bodyLarge?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
          textAlign: TextAlign.center,
        ),
      ),
    );
  }
}

class _PeerCard extends StatelessWidget {
  const _PeerCard({required this.peer, required this.onTap});

  final PeerView peer;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: EdgeInsets.zero,
      clipBehavior: Clip.antiAlias,
      child: ListTile(
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 18,
          vertical: 10,
        ),
        leading: Icon(_platformIcon(peer.platform), size: 30),
        title: Text(peer.name),
        subtitle: Padding(
          padding: const EdgeInsets.only(top: 4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text('Bluetooth で発見'),
              Text(peer.routeEstimate.label),
            ],
          ),
        ),
        onTap: onTap,
      ),
    );
  }

  IconData _platformIcon(PeerPlatform platform) {
    switch (platform) {
      case PeerPlatform.ios:
      case PeerPlatform.android:
        return Icons.smartphone;
      case PeerPlatform.windows:
      case PeerPlatform.macos:
      case PeerPlatform.linux:
        return Icons.computer;
    }
  }
}
